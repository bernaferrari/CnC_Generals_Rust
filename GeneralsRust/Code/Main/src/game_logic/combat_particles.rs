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
use std::collections::{HashMap, HashSet};

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
    /// In-flight projectile exhaust residual (Weapon.ini ProjectileExhaust).
    ProjectileExhaust,
    /// C++ `ParticleSysBone` attached to a live drawable (exhaust, stacks, clouds).
    ParticleSysBone,
    /// C++ ActiveBody AutoFire bone fire (FIRESMALL/MEDIUM/LARGE).
    BodyFire,
    /// C++ ActiveBody AutoSmoke bone smoke (SMOKESMALL/MEDIUM/LARGE).
    BodySmoke,
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
            CombatParticleKind::ProjectileExhaust => "MissileExhaust",
            CombatParticleKind::ParticleSysBone => "SmokePlume",
            CombatParticleKind::BodyFire => "FireSmall",
            CombatParticleKind::BodySmoke => "SmokeSmall",
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
    /// Client particle-system ids created by this registry in the current run.
    ///
    /// This is deliberately not part of a snapshot. A saved client id is only a
    /// historical handle and must never be destroyed by a newly restored host
    /// registry; live projectile exhausts rebind on their next update.
    client_system_ids: HashSet<u32>,
    /// Destruction notifications for PresentationFrame events this frame.
    destroyed_this_frame: Vec<(ObjectId, Team)>,
    /// Particle ids spawned this frame (presentation event drain).
    spawned_this_frame: Vec<u32>,
    /// C++ `setMuzzleFlashHidden(false)` for one RecoilStart frame.
    muzzle_flash_until: HashMap<ObjectId, u32>,
}

impl CombatParticleRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            systems: HashMap::new(),
            client_system_ids: HashSet::new(),
            destroyed_this_frame: Vec::new(),
            spawned_this_frame: Vec::new(),
            muzzle_flash_until: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        for client_system_id in self.client_system_ids.drain() {
            mirror_destroy_client_system(client_system_id);
        }
        self.systems.clear();
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
        self.muzzle_flash_until.clear();
        self.next_id = 1;
    }

    /// Allocator cursor for next system id (survives save/load).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Replace registry contents from a save/load snapshot.
    ///
    /// Frame-local drains (`destroyed_this_frame` / `spawned_this_frame`) are
    /// cleared. Client mirror ids are preserved as stored (they are not treated
    /// as owned after load, so a live projectile exhaust safely rebinds).
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

    pub fn get_mut(&mut self, id: u32) -> Option<&mut CombatParticleSystemEntry> {
        self.systems.get_mut(&id)
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
        self.spawn_with_template(
            kind,
            kind.template_name().to_string(),
            position,
            frame,
            source,
            target,
        )
    }

    /// Spawn a named particle-system template (DisableFX BinaryShower, etc.).
    pub fn spawn_named(
        &mut self,
        kind: CombatParticleKind,
        template_name: impl Into<String>,
        position: Vec3,
        frame: u32,
        source: Option<ObjectId>,
        target: Option<ObjectId>,
    ) -> u32 {
        self.spawn_with_template(kind, template_name.into(), position, frame, source, target)
    }


    /// Spawn with an exact particle-system template rather than the generic
    /// kind preset. `ProjectileExhaust` needs this because Weapon.ini names a
    /// ParticleSystem template directly; it is not an FXList entry.
    fn spawn_with_template(
        &mut self,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
        frame: u32,
        source: Option<ObjectId>,
        target: Option<ObjectId>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);

        let client_system_id = mirror_spawn_to_client_manager(&template_name, position);
        if let Some(client_system_id) = client_system_id {
            self.client_system_ids.insert(client_system_id);
        }

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
    ///
    /// C++ `Weapon::fireWeaponTemplate` plays the authored FireFX list at the
    /// barrel, never a generic `MuzzleFlash` preset. ParticleSystem nuggets
    /// inside that list become the registry templates. `victim` is passed as
    /// FXList secondary so Tracer/RayEffect nuggets run.
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
        self.note_muzzle_flash_unhide(shooter, frame);
        let mut ids = Vec::new();
        if let Some(fx) = usable_particle_template_name(fire_fx_name) {
            let authored = crate::game_logic::particle_template_names_for_fx_list(fx);
            if authored.is_empty() {
                let muzzle_id = self.spawn_fx_list_marker(
                    CombatParticleKind::WeaponMuzzleFlash,
                    fx,
                    muzzle_pos,
                    frame,
                    Some(shooter),
                    target,
                );
                if !fire_ocl_name.is_empty() {
                    if let Some(e) = self.systems.get_mut(&muzzle_id) {
                        e.ocl_list_name = fire_ocl_name.to_string();
                    }
                }
                ids.push(muzzle_id);
            } else {
                for (index, template) in authored.iter().enumerate() {
                    let muzzle_id = self.spawn_with_template(
                        CombatParticleKind::WeaponMuzzleFlash,
                        template.clone(),
                        muzzle_pos,
                        frame,
                        Some(shooter),
                        target,
                    );
                    if let Some(e) = self.systems.get_mut(&muzzle_id) {
                        e.fx_list_name = fx.to_string();
                        if index == 0 && !fire_ocl_name.is_empty() {
                            e.ocl_list_name = fire_ocl_name.to_string();
                        }
                    }
                    ids.push(muzzle_id);
                }
            }
            let _ = crate::game_logic::dispatch_fx_list_at_pos_ex(
                fx,
                muzzle_pos,
                impact_pos,
                0.0,
                0.0,
            );
        } else {
            let muzzle_id = self.spawn(
                CombatParticleKind::WeaponMuzzleFlash,
                muzzle_pos,
                frame,
                Some(shooter),
                target,
            );
            if !fire_ocl_name.is_empty() {
                if let Some(e) = self.systems.get_mut(&muzzle_id) {
                    e.ocl_list_name = fire_ocl_name.to_string();
                }
            }
            ids.push(muzzle_id);
        }
        if let Some(impact) = impact_pos {
            if let Some(fx) = usable_particle_template_name(detonation_fx_name) {
                let authored = crate::game_logic::particle_template_names_for_fx_list(fx);
                let impact_id = if let Some(template) = authored.first() {
                    self.spawn_with_template(
                        CombatParticleKind::WeaponImpact,
                        template.clone(),
                        impact,
                        frame,
                        Some(shooter),
                        target,
                    )
                } else {
                    self.spawn_fx_list_marker(
                        CombatParticleKind::WeaponImpact,
                        fx,
                        impact,
                        frame,
                        Some(shooter),
                        target,
                    )
                };
                if let Some(e) = self.systems.get_mut(&impact_id) {
                    e.fx_list_name = fx.to_string();
                    if !detonation_ocl_name.is_empty() {
                        e.ocl_list_name = detonation_ocl_name.to_string();
                    }
                }
                let _ = crate::game_logic::dispatch_fx_list_at_pos_ex(
                    fx,
                    impact,
                    Some(impact),
                    0.0,
                    0.0,
                );
                ids.push(impact_id);
            } else {
                let impact_id = self.spawn(
                    CombatParticleKind::WeaponImpact,
                    impact,
                    frame,
                    Some(shooter),
                    target,
                );
                if !detonation_ocl_name.is_empty() {
                    if let Some(e) = self.systems.get_mut(&impact_id) {
                        e.ocl_list_name = detonation_ocl_name.to_string();
                    }
                }
                ids.push(impact_id);
            }
        }
        ids
    }

    /// Host marker for an authored FXList that is not itself a ParticleSystem.
    /// Skips the generic preset client mirror (C++ never creates `MuzzleFlash`).
    fn spawn_fx_list_marker(
        &mut self,
        kind: CombatParticleKind,
        fx_list_name: &str,
        position: Vec3,
        frame: u32,
        source: Option<ObjectId>,
        target: Option<ObjectId>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let entry = CombatParticleSystemEntry {
            id,
            kind,
            template_name: fx_list_name.to_string(),
            position,
            source_object: source,
            target_object: target,
            spawned_frame: frame,
            active: true,
            client_system_id: None,
            fx_list_name: fx_list_name.to_string(),
            ocl_list_name: String::new(),
        };
        self.systems.insert(id, entry);
        self.spawned_this_frame.push(id);
        id
    }

    /// C++ `W3DModelDraw::handleWeaponFireFX` `setMuzzleFlashHidden(false)`.
    pub fn note_muzzle_flash_unhide(&mut self, source: ObjectId, frame: u32) {
        self.muzzle_flash_until.insert(source, frame.saturating_add(1));
    }

    /// True while the live drawable should show muzzle-flash subobjects.
    pub fn muzzle_flash_is_visible(&self, source: ObjectId, frame: u32) -> bool {
        self.muzzle_flash_until
            .get(&source)
            .is_some_and(|until| frame <= *until)
    }

    /// C++ ProjectileExhaust residual: in-flight trail particle at projectile pos.
    ///
    /// C++ `WeaponTemplate::fireProjectile` passes one template to the
    /// projectile, and `MissileAIUpdate::doIgnitionState` creates one attached
    /// particle system. Keep the same one-system-per-projectile lifetime here;
    /// the host projectile has no client drawable to attach to, so its world
    /// position is synchronized instead.
    pub fn spawn_projectile_exhaust(
        &mut self,
        position: Vec3,
        frame: u32,
        shooter: ObjectId,
        projectile_id: Option<ObjectId>,
        exhaust_name: &str,
    ) -> Option<u32> {
        let exhaust_name = usable_particle_template_name(exhaust_name)?;
        if let Some(projectile_id) = projectile_id {
            return Some(self.upsert_projectile_exhaust(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
            ));
        }

        let id = self.spawn_with_template(
            CombatParticleKind::ProjectileExhaust,
            exhaust_name.to_string(),
            position,
            frame,
            Some(shooter),
            None,
        );
        Some(id)
    }

    /// Synchronize named projectile exhausts to the current projectile set.
    ///
    /// The C++ missile owns a single attached particle system from ignition to
    /// detonation. The old host residual spawned a new system each logic frame,
    /// leaving every old trail alive. This updates the one live entry and tears
    /// it down as soon as its projectile is absent from the combat snapshot.
    pub fn sync_projectile_exhausts(
        &mut self,
        frame: u32,
        projectiles: &[(ObjectId, ObjectId, Vec3, String)],
    ) {
        let live_projectiles: HashSet<ObjectId> = projectiles
            .iter()
            .map(|(projectile_id, _, _, _)| *projectile_id)
            .collect();

        for (projectile_id, shooter, position, exhaust_name) in projectiles {
            if let Some(exhaust_name) = usable_particle_template_name(exhaust_name) {
                self.upsert_projectile_exhaust(
                    *position,
                    frame,
                    *shooter,
                    *projectile_id,
                    exhaust_name,
                );
            } else {
                self.deactivate_projectile_exhausts_for(*projectile_id);
            }
        }

        let stale_ids: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && entry.kind == CombatParticleKind::ProjectileExhaust
                    && entry
                        .target_object
                        .is_some_and(|projectile_id| !live_projectiles.contains(&projectile_id))
            })
            .map(|entry| entry.id)
            .collect();
        for id in stale_ids {
            self.deactivate(id);
        }
    }

    fn upsert_projectile_exhaust(
        &mut self,
        position: Vec3,
        frame: u32,
        shooter: ObjectId,
        projectile_id: ObjectId,
        exhaust_name: &str,
    ) -> u32 {
        let mut matching: Vec<(u32, u32)> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && entry.kind == CombatParticleKind::ProjectileExhaust
                    && entry.target_object == Some(projectile_id)
            })
            .map(|entry| (entry.spawned_frame, entry.id))
            .collect();
        matching.sort_unstable();

        let existing_id = matching.pop().map(|(_, id)| id);
        for (_, duplicate_id) in matching {
            self.deactivate(duplicate_id);
        }

        let Some(existing_id) = existing_id else {
            return self.spawn_projectile_exhaust_entry(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
            );
        };

        let template_matches = self
            .systems
            .get(&existing_id)
            .is_some_and(|entry| entry.template_name == exhaust_name);
        if !template_matches {
            self.deactivate(existing_id);
            return self.spawn_projectile_exhaust_entry(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
            );
        }

        let current_client_id = self
            .systems
            .get(&existing_id)
            .and_then(|entry| entry.client_system_id);
        if let Some(client_system_id) = current_client_id
            .filter(|client_system_id| self.client_system_ids.contains(client_system_id))
        {
            mirror_update_client_system_position(client_system_id, position);
        } else {
            // Snapshot ids are not safe to destroy or update. Recreate exactly
            // this template and mark only the new id as host-owned.
            if let Some(client_system_id) = current_client_id {
                self.client_system_ids.remove(&client_system_id);
            }
            let client_system_id = mirror_spawn_to_client_manager(exhaust_name, position);
            if let Some(client_system_id) = client_system_id {
                self.client_system_ids.insert(client_system_id);
            }
            if let Some(entry) = self.systems.get_mut(&existing_id) {
                entry.client_system_id = client_system_id;
            }
        }

        if let Some(entry) = self.systems.get_mut(&existing_id) {
            entry.position = position;
            entry.source_object = Some(shooter);
            // `ProjectileExhaust` is a ParticleSystem name, not an FXList name.
            entry.fx_list_name.clear();
        }
        existing_id
    }

    fn spawn_projectile_exhaust_entry(
        &mut self,
        position: Vec3,
        frame: u32,
        shooter: ObjectId,
        projectile_id: ObjectId,
        exhaust_name: &str,
    ) -> u32 {
        self.spawn_with_template(
            CombatParticleKind::ProjectileExhaust,
            exhaust_name.to_string(),
            position,
            frame,
            Some(shooter),
            Some(projectile_id),
        )
    }

    fn deactivate_projectile_exhausts_for(&mut self, projectile_id: ObjectId) {
        let ids: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && entry.kind == CombatParticleKind::ProjectileExhaust
                    && entry.target_object == Some(projectile_id)
            })
            .map(|entry| entry.id)
            .collect();
        for id in ids {
            self.deactivate(id);
        }
    }

    /// C++ `ObjectCreationList.cpp:962-969` attach ParticleSystem to a spawned object.
    pub fn attach_named_to_object(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        template_name: &str,
    ) -> Option<u32> {
        let template_name = usable_particle_template_name(template_name)?;
        let id = self.spawn_with_template(
            CombatParticleKind::ParticleSysBone,
            template_name.to_string(),
            position,
            frame,
            Some(owner),
            None,
        );
        let leftover_id = gamelogic::helpers::attach_particle_system_to_object(
            template_name,
            owner.0,
        );
        if leftover_id.is_some() {
            if let Some(entry) = self.systems.get_mut(&id) {
                if entry.client_system_id.is_none() {
                    entry.client_system_id = leftover_id;
                    if let Some(client_id) = leftover_id {
                        self.client_system_ids.insert(client_id);
                    }
                }
            }
        }
        Some(id)
    }

    /// C++ `W3DModelDraw::recalcBonesForClientParticleSystems` for one object.
    pub fn sync_particle_sys_bones(
        &mut self,
        frame: u32,
        owner: ObjectId,
        position: Vec3,
        bones: &[(String, String)],
    ) {
        let wanted: HashSet<String> = bones
            .iter()
            .filter_map(|(_, system)| usable_particle_template_name(system).map(str::to_string))
            .collect();
        let stale: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && entry.kind == CombatParticleKind::ParticleSysBone
                    && entry.source_object == Some(owner)
                    && !wanted.contains(&entry.template_name)
            })
            .map(|entry| entry.id)
            .collect();
        for id in stale {
            self.deactivate(id);
        }
        for system in wanted {
            let existing = self.systems.values().find_map(|entry| {
                (entry.active
                    && entry.kind == CombatParticleKind::ParticleSysBone
                    && entry.source_object == Some(owner)
                    && entry.template_name == system)
                    .then_some((entry.id, entry.client_system_id))
            });
            if let Some((id, client_id)) = existing {
                if let Some(client_id) = client_id {
                    if self.client_system_ids.contains(&client_id) {
                        mirror_update_client_system_position(client_id, position);
                    }
                }
                if let Some(entry) = self.systems.get_mut(&id) {
                    entry.position = position;
                }
            } else {
                let _ = self.attach_named_to_object(owner, position, frame, &system);
            }
        }
    }

    /// C++ `ActiveBody::updateBodyParticleSystems` on body-state change.
    pub fn replace_body_auto_particles(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        body_ordinal: u8,
        aflame: bool,
    ) {
        let stale: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && matches!(
                        entry.kind,
                        CombatParticleKind::BodyFire | CombatParticleKind::BodySmoke
                    )
                    && entry.source_object == Some(owner)
            })
            .map(|entry| entry.id)
            .collect();
        for id in stale {
            self.deactivate(id);
        }
        // Pristine models typically have no FIRESMALL/SMOKESMALL bones.
        if body_ordinal == 0 && !aflame {
            return;
        }
        for (kind, template) in body_auto_particle_templates(aflame) {
            let Some(template) = usable_particle_template_name(template.as_str()) else {
                continue;
            };
            let id = self.spawn_with_template(
                kind,
                template.to_string(),
                position,
                frame,
                Some(owner),
                None,
            );
            let leftover_id =
                gamelogic::helpers::attach_particle_system_to_object(template, owner.0);
            if leftover_id.is_some() {
                if let Some(entry) = self.systems.get_mut(&id) {
                    if entry.client_system_id.is_none() {
                        entry.client_system_id = leftover_id;
                        if let Some(client_id) = leftover_id {
                            self.client_system_ids.insert(client_id);
                        }
                    }
                }
            }
        }
    }

    pub fn has_body_particles(&self, owner: ObjectId) -> bool {
        self.systems.values().any(|entry| {
            entry.active
                && matches!(
                    entry.kind,
                    CombatParticleKind::BodyFire | CombatParticleKind::BodySmoke
                )
                && entry.source_object == Some(owner)
        })
    }

    pub fn follow_attached_body_particles(&mut self, owner: ObjectId, position: Vec3) {
        let ids: Vec<(u32, Option<u32>)> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && matches!(
                        entry.kind,
                        CombatParticleKind::BodyFire | CombatParticleKind::BodySmoke
                    )
                    && entry.source_object == Some(owner)
            })
            .map(|entry| (entry.id, entry.client_system_id))
            .collect();
        for (id, client_id) in ids {
            if let Some(client_id) = client_id {
                if self.client_system_ids.contains(&client_id) {
                    mirror_update_client_system_position(client_id, position);
                }
            }
            if let Some(entry) = self.systems.get_mut(&id) {
                entry.position = position;
            }
        }
    }

    pub fn deactivate(&mut self, id: u32) {
        let client_system_id = match self.systems.get_mut(&id) {
            Some(entry) if entry.active => {
                entry.active = false;
                entry.client_system_id.take()
            }
            _ => None,
        };
        if let Some(client_system_id) = client_system_id {
            if self.client_system_ids.remove(&client_system_id) {
                mirror_destroy_client_system(client_system_id);
            }
        }
    }
}

/// C++ `INI::parseParticleSystemTemplate` treats `None` as a null template.
/// Keep that sentinel out of the runtime registry rather than inventing a
/// system named `None`.
fn usable_particle_template_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then_some(name)
}

fn body_auto_particle_templates(aflame: bool) -> Vec<(CombatParticleKind, String)> {
    let (fire, smoke) = match game_engine::common::global_data::read_safe() {
        Ok(data) => {
            let fire = if aflame {
                first_nonempty(&[
                    data.auto_fire_particle_medium_system.as_str(),
                    data.auto_fire_particle_small_system.as_str(),
                    "FireMedium",
                ])
            } else {
                first_nonempty(&[
                    data.auto_fire_particle_small_system.as_str(),
                    "FireSmall",
                ])
            };
            let smoke = if aflame {
                first_nonempty(&[
                    data.auto_fire_particle_small_system.as_str(),
                    "FireSmall",
                ])
            } else {
                first_nonempty(&[
                    data.auto_smoke_particle_small_system.as_str(),
                    "SmokeSmall",
                ])
            };
            (fire, smoke)
        }
        Err(_) => {
            if aflame {
                ("FireMedium".to_string(), "FireSmall".to_string())
            } else {
                ("FireSmall".to_string(), "SmokeSmall".to_string())
            }
        }
    };
    vec![
        (CombatParticleKind::BodyFire, fire),
        (CombatParticleKind::BodySmoke, smoke),
    ]
}

fn first_nonempty(names: &[&str]) -> String {
    names
        .iter()
        .find(|name| usable_particle_template_name(name).is_some())
        .unwrap_or(&names[names.len() - 1])
        .to_string()
}

/// C++ current-state `ParticleSysBone` list for a live host object.
pub fn particle_sys_bones_for_template(
    template_name: &str,
    condition_bits: u128,
) -> Vec<(String, String)> {
    let mut bones = Vec::new();
    if let Some(manager) = crate::assets::get_asset_manager() {
        if let Ok(guard) = manager.lock() {
            if let Some(definition) = guard
                .get_object_definition(template_name)
                .or_else(|| guard.resolve_object_definition(template_name, None))
            {
                bones.extend(definition.particle_sys_bones_for_conditions(condition_bits));
            }
        }
    }
    if bones.is_empty() {
        if template_name.contains("RepairVehiclesInArea") {
            bones.push((
                "NONE".to_string(),
                crate::game_logic::host_emergency_repair::EMERGENCY_REPAIR_CLOUD_PARTICLE
                    .to_string(),
            ));
        }
        if template_name.contains("Frenzy") && template_name.contains("Marker") {
            bones.push((
                "NONE".to_string(),
                crate::game_logic::host_frenzy::FRENZY_CLOUD_PARTICLE.to_string(),
            ));
        }
    }
    bones
}

fn mirror_update_client_system_position(system_id: u32, position: Vec3) {
    let position = gamelogic::common::Coord3D::new(position.x, position.y, position.z);
    if let Some(manager) = gamelogic::helpers::TheParticleSystemManager::get() {
        manager.set_particle_system_position(system_id, &position);
    }
}

fn mirror_destroy_client_system(system_id: u32) {
    if let Some(manager) = gamelogic::helpers::TheParticleSystemManager::get() {
        manager.destroy_particle_system(system_id);
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
            register_particle_system_manager_bridge, ParticleSystemManager,
        };

        // Position/destroy updates go through GameLogic's C++-shaped bridge.
        // Register it here too because combat can begin before full GameClient
        // initialization in a host-driven match startup.
        register_particle_system_manager_bridge();

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

    #[test]
    fn projectile_exhaust_reuses_one_named_system_and_stops_with_projectile() {
        let mut reg = CombatParticleRegistry::new();
        let projectile = ObjectId(700);
        let shooter = ObjectId(42);
        let first_position = Vec3::new(4.0, 2.0, 8.0);

        reg.sync_projectile_exhausts(
            10,
            &[(
                projectile,
                shooter,
                first_position,
                "TowMissileExhaust".to_string(),
            )],
        );
        let exhaust = reg
            .systems_of_kind(CombatParticleKind::ProjectileExhaust)
            .pop()
            .expect("one live projectile exhaust");
        let exhaust_id = exhaust.id;
        assert_eq!(exhaust.template_name, "TowMissileExhaust");
        assert!(exhaust.fx_list_name.is_empty());
        assert_eq!(reg.spawned_this_frame(), &[exhaust_id]);

        let moved_position = Vec3::new(12.0, 3.0, 20.0);
        reg.clear_frame_events();
        reg.sync_projectile_exhausts(
            11,
            &[(
                projectile,
                shooter,
                moved_position,
                "TowMissileExhaust".to_string(),
            )],
        );
        let exhausts = reg.systems_of_kind(CombatParticleKind::ProjectileExhaust);
        assert_eq!(
            exhausts.len(),
            1,
            "one C++-style attached exhaust per missile"
        );
        assert_eq!(
            exhausts[0].id, exhaust_id,
            "trail keeps its identity while flying"
        );
        assert_eq!(exhausts[0].position, moved_position);
        assert_eq!(
            reg.spawned_this_frame(),
            &[] as &[u32],
            "a position update must not create another particle system"
        );

        let no_projectiles: [(ObjectId, ObjectId, Vec3, String); 0] = [];
        reg.sync_projectile_exhausts(12, &no_projectiles);
        assert!(
            !reg.get(exhaust_id).expect("retained history entry").active,
            "trail is destroyed when the projectile impacts or expires"
        );
        assert!(reg
            .systems_of_kind(CombatParticleKind::ProjectileExhaust)
            .is_empty());

        reg.sync_projectile_exhausts(
            13,
            &[(ObjectId(701), shooter, Vec3::ZERO, "None".to_string())],
        );
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::ProjectileExhaust)
                .len(),
            0,
            "the C++ ParticleSystem `None` sentinel must not become a live trail"
        );
    }

    #[test]
    fn authored_fire_fx_is_not_generic_muzzle_preset() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named(
            Vec3::new(4.0, 1.0, 8.0),
            Some(Vec3::new(10.0, 0.0, 10.0)),
            7,
            ObjectId(3),
            Some(ObjectId(4)),
            "WeaponFX_GenericTankGunNoTracer",
            "",
        );
        assert!(!ids.is_empty());
        let muzzle = reg.get(ids[0]).expect("muzzle");
        assert_eq!(muzzle.fx_list_name, "WeaponFX_GenericTankGunNoTracer");
        assert_ne!(
            muzzle.template_name, "MuzzleFlash",
            "authored FireFX must not spawn the generic MuzzleFlash preset"
        );
        assert!(reg.muzzle_flash_is_visible(ObjectId(3), 7));
        assert!(reg.muzzle_flash_is_visible(ObjectId(3), 8));
        assert!(!reg.muzzle_flash_is_visible(ObjectId(3), 9));
    }

    #[test]
    fn particle_sys_bones_and_body_fire_attach() {
        let mut reg = CombatParticleRegistry::new();
        let owner = ObjectId(11);
        let pos = Vec3::new(2.0, 0.0, 3.0);
        reg.sync_particle_sys_bones(
            4,
            owner,
            pos,
            &[("exhaust01".to_string(), "DieselSmoke".to_string())],
        );
        let bones = reg.systems_of_kind(CombatParticleKind::ParticleSysBone);
        assert_eq!(bones.len(), 1);
        assert_eq!(bones[0].template_name, "DieselSmoke");
        assert_eq!(bones[0].source_object, Some(owner));

        reg.replace_body_auto_particles(owner, pos, 5, 1, false);
        assert!(reg.has_body_particles(owner));
        assert!(!reg
            .systems_of_kind(CombatParticleKind::BodyFire)
            .is_empty());
        assert!(!reg
            .systems_of_kind(CombatParticleKind::BodySmoke)
            .is_empty());

        reg.replace_body_auto_particles(owner, pos, 6, 0, false);
        assert!(!reg.has_body_particles(owner));

        let attached = reg
            .attach_named_to_object(owner, pos, 7, "WreckSmoke")
            .expect("ocl particle");
        assert_eq!(
            reg.get(attached).expect("entry").template_name,
            "WreckSmoke"
        );
    }

    #[test]
    fn repair_and_frenzy_markers_have_cloud_bones() {
        let repair = particle_sys_bones_for_template(
            "RepairVehiclesInArea_InvisibleMarker_Level1",
            0,
        );
        assert!(repair
            .iter()
            .any(|(_, system)| system == "RepairCloud"));
        let frenzy = particle_sys_bones_for_template("Frenzy_InvisibleMarker", 0);
        assert!(frenzy.iter().any(|(_, system)| system == "FrenzyCloud"));
    }
}
