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
pub use generals_game_domain::CombatParticleKind;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[path = "combat_particle_bindings.rs"]
mod client_bindings;
pub use client_bindings::ParticleBindingAccess;
pub(crate) use client_bindings::ReceivingParticlePoseAccess;
use client_bindings::{
    can_retire_bindings, retire_client_binding, spawn_client_binding, ClientParticleBinding,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CombatParticleAttachment {
    pub owner: ObjectId,
    pub local_offset: Vec3,
    pub lifetime: Option<u32>,
    pub local_z_rotation: f32,
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
    /// Host-local bone offset from the owning object origin (Y-up).
    /// C++ `ParticleSystem::setPosition` + `attachToObject` local bone space.
    #[serde(default)]
    pub attach_offset: Vec3,
    /// Rebound from receiving save metadata; not positional historical bytes.
    #[serde(skip)]
    pub(crate) attachment: Option<CombatParticleAttachment>,
}

/// Lightweight host particle system registry for combat/build feedback.
///
/// Independent of WGPU so unit tests and headless golden host paths can assert
/// that kills/fires produce registry entries.
#[derive(Debug)]
pub struct CombatParticleRegistry {
    next_id: u32,
    systems: HashMap<u32, CombatParticleSystemEntry>,
    /// Actual manager namespace receipts keyed by canonical host particle ID.
    ///
    /// This is deliberately not part of a snapshot. A saved client id is only a
    /// historical handle and must never be destroyed by a newly restored host
    /// registry; live projectile exhausts rebind on their next update.
    client_bindings: HashMap<u32, ClientParticleBinding>,
    /// Destruction notifications for PresentationFrame events this frame.
    destroyed_this_frame: Vec<(ObjectId, Team)>,
    /// Particle ids spawned this frame (presentation event drain).
    spawned_this_frame: Vec<u32>,
    /// C++ `setMuzzleFlashHidden(false)` for one RecoilStart frame.
    muzzle_flash_until: HashMap<ObjectId, u32>,
    /// Newest logic frame observed through spawns / per-frame syncs. The
    /// expiry sweep in `clear_frame_events` measures elapsed lifetimes
    /// against it.
    current_frame: u32,
    /// Immutable application GameData baseline plus the current map projection.
    body_auto_particle_base: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    body_auto_particle_effective: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
}

impl Default for CombatParticleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl super::GameLogic {
    pub(crate) fn body_auto_particle_baseline(
        &self,
    ) -> &game_engine::common::ini::ini_game_data::BodyAutoParticleRules {
        self.combat_particles.body_auto_particle_baseline()
    }

    pub(crate) fn admit_body_auto_particle_baseline(
        &mut self,
        rules: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    ) {
        self.combat_particles
            .admit_body_auto_particle_baseline(rules);
    }
}

impl CombatParticleRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            systems: HashMap::new(),
            client_bindings: HashMap::new(),
            destroyed_this_frame: Vec::new(),
            spawned_this_frame: Vec::new(),
            muzzle_flash_until: HashMap::new(),
            current_frame: 0,
            body_auto_particle_base: Default::default(),
            body_auto_particle_effective: Default::default(),
        }
    }

    pub(crate) fn admit_body_auto_particle_baseline(
        &mut self,
        rules: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    ) {
        self.body_auto_particle_base = rules.clone();
        self.body_auto_particle_effective = rules;
    }

    pub(crate) fn body_auto_particle_baseline(
        &self,
    ) -> &game_engine::common::ini::ini_game_data::BodyAutoParticleRules {
        &self.body_auto_particle_base
    }

    /// Effective Body particle definitions for this receiving session.
    pub fn body_auto_particle_rules(
        &self,
    ) -> &game_engine::common::ini::ini_game_data::BodyAutoParticleRules {
        &self.body_auto_particle_effective
    }

    pub(crate) fn apply_body_auto_particle_map_rules(
        &mut self,
        rules: game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
    ) {
        self.body_auto_particle_effective = rules;
    }

    pub(crate) fn reset_body_auto_particle_map_rules(&mut self) {
        self.body_auto_particle_effective = self.body_auto_particle_base.clone();
    }

    pub(crate) fn can_clear(&self) -> bool {
        can_retire_bindings(&self.client_bindings)
    }

    pub fn clear(&mut self) -> Result<(), String> {
        if !self.can_clear() {
            return Err("particle registry requires its actual live receiving manager".into());
        }
        for (_, binding) in self.client_bindings.drain() {
            let retired = retire_client_binding(&binding, &mut ());
            debug_assert!(
                retired,
                "driving particle owner changed during synchronous clear"
            );
        }
        self.systems.clear();
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
        self.muzzle_flash_until.clear();
        self.next_id = 1;
        Ok(())
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
    ) -> Result<(), String> {
        self.clear()?;
        let mut max_id = 0_u32;
        for entry in systems {
            max_id = max_id.max(entry.id);
            self.systems.insert(entry.id, entry);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
        Ok(())
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
        // Per-frame expiry sweep (called once per host logic tick): one-shot
        // systems deactivate once their system_lifetime elapses since spawn so
        // the registry cannot grow unbounded.
        self.sweep_expired_systems(self.current_frame);
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
    }

    /// C++ finite `SystemLifetime` sweep (ParticleSys.cpp:2066-2081):
    /// deactivate one-shot feedback systems once their lifetime has elapsed
    /// since spawn. Attached/continuous kinds (exhausts, bone systems, body
    /// fire/smoke) are owned by object updates and never expire here.
    pub fn sweep_expired_systems(&mut self, frame: u32) {
        let expired: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                if self.client_bindings.contains_key(&entry.id) {
                    return false;
                }
                let Some(lifetime) = entry.kind.sweep_system_lifetime() else {
                    return false;
                };
                entry.active && frame.saturating_sub(entry.spawned_frame) >= lifetime
            })
            .map(|entry| entry.id)
            .collect();
        for id in expired {
            self.deactivate(id, &mut ());
        }
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
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> u32 {
        let id =
            self.spawn_with_template(kind, template_name.into(), position, frame, source, target);
        self.bind_source_entry(id, particle_owner);
        id
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
        self.current_frame = self.current_frame.max(frame);
        self.next_id = self.next_id.saturating_add(1).max(1);

        let mut entry = CombatParticleSystemEntry {
            id,
            kind,
            template_name,
            position,
            source_object: source,
            target_object: target,
            spawned_frame: frame,
            active: true,
            client_system_id: None,
            fx_list_name: String::new(),
            ocl_list_name: String::new(),
            attach_offset: Vec3::ZERO,
            attachment: None,
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
            0.0,
            0.0,
        )
    }

    /// Weapon fire FX + OCL residual names (FireOCL at muzzle, DetonationOCL at impact).
    ///
    /// C++ `Weapon::fireWeaponTemplate` plays the authored FireFX list at the
    /// barrel, never a generic `MuzzleFlash` preset. ParticleSystem nuggets
    /// inside that list become the registry templates. `victim` is passed as
    /// FXList secondary so Tracer/RayEffect nuggets run. `override_radius` is
    /// C++ `getPrimaryDamageRadius(bonus)` so `UseCallersRadius` nuggets scale
    /// sphere/cylinder emission to the weapon splash.
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
        primary_speed: f32,
        override_radius: f32,
    ) -> Vec<u32> {
        self.spawn_weapon_fire_fx_named_ocl_oriented(
            muzzle_pos,
            impact_pos,
            frame,
            shooter,
            target,
            fire_fx_name,
            detonation_fx_name,
            fire_ocl_name,
            detonation_ocl_name,
            primary_speed,
            override_radius,
            None,
        )
    }

    /// Same as [`Self::spawn_weapon_fire_fx_named_ocl`] with C++ drawable matrix.
    pub fn spawn_weapon_fire_fx_named_ocl_oriented(
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
        primary_speed: f32,
        override_radius: f32,
        drawable_matrix: Option<glam::Mat4>,
    ) -> Vec<u32> {
        self.spawn_weapon_fire_fx_named_ocl_oriented_with_dispatch(
            muzzle_pos,
            impact_pos,
            frame,
            shooter,
            target,
            fire_fx_name,
            detonation_fx_name,
            fire_ocl_name,
            detonation_ocl_name,
            primary_speed,
            override_radius,
            drawable_matrix,
            &mut |name, position, secondary, speed, radius, matrix| {
                crate::game_logic::dispatch_fx_list_at_pos_oriented(
                    name, position, secondary, speed, radius, matrix,
                )
            },
        )
    }

    /// Immediate dispatch supplied by the driving session; no callback escapes.
    #[cfg(feature = "game_client")]
    pub fn spawn_weapon_fire_fx_named_ocl_oriented_with_view(
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
        primary_speed: f32,
        override_radius: f32,
        drawable_matrix: Option<glam::Mat4>,
        view: &mut crate::game_logic::host_fx_list_dispatch::ObjectFxServices<'_, '_>,
    ) -> Vec<u32> {
        self.spawn_weapon_fire_fx_named_ocl_oriented_with_dispatch(
            muzzle_pos,
            impact_pos,
            frame,
            shooter,
            target,
            fire_fx_name,
            detonation_fx_name,
            fire_ocl_name,
            detonation_ocl_name,
            primary_speed,
            override_radius,
            drawable_matrix,
            &mut |name, position, secondary, speed, radius, matrix| {
                crate::game_logic::GameLogic::dispatch_fx_list_at_position_with_view(
                    name, position, secondary, speed, radius, matrix, view,
                )
            },
        )
    }

    pub fn spawn_weapon_fire_fx_named_ocl_oriented_with_dispatch(
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
        primary_speed: f32,
        override_radius: f32,
        drawable_matrix: Option<glam::Mat4>,
        dispatch_fx: &mut dyn FnMut(&str, Vec3, Option<Vec3>, f32, f32, Option<glam::Mat4>) -> bool,
    ) -> Vec<u32> {
        self.note_muzzle_flash_unhide(shooter, frame);
        let mut ids = Vec::new();
        if let Some(fx) = usable_particle_template_name(fire_fx_name) {
            // C++ Weapon::fireWeaponTemplate plays FireFX exactly once via
            // FXList::doFXPos. Host registry keeps a marker; ParticleSystem
            // nuggets are created only by dispatch (not spawn_with_template).
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
            let _ = dispatch_fx(
                fx,
                muzzle_pos,
                impact_pos,
                primary_speed,
                override_radius,
                drawable_matrix,
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
                let impact_id = self.spawn_fx_list_marker(
                    CombatParticleKind::WeaponImpact,
                    fx,
                    impact,
                    frame,
                    Some(shooter),
                    target,
                );
                if let Some(e) = self.systems.get_mut(&impact_id) {
                    if !detonation_ocl_name.is_empty() {
                        e.ocl_list_name = detonation_ocl_name.to_string();
                    }
                }
                let _ = dispatch_fx(
                    fx,
                    impact,
                    Some(impact),
                    0.0,
                    override_radius,
                    drawable_matrix,
                );
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
        self.current_frame = self.current_frame.max(frame);
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
            attach_offset: Vec3::ZERO,
            attachment: None,
        };
        self.systems.insert(id, entry);
        self.spawned_this_frame.push(id);
        id
    }

    /// C++ `W3DModelDraw::handleWeaponFireFX` `setMuzzleFlashHidden(false)`.
    pub fn note_muzzle_flash_unhide(&mut self, source: ObjectId, frame: u32) {
        self.muzzle_flash_until
            .insert(source, frame.saturating_add(1));
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
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> Option<u32> {
        let exhaust_name = usable_particle_template_name(exhaust_name)?;
        if let Some(projectile_id) = projectile_id {
            return Some(self.upsert_projectile_exhaust(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
                particle_owner,
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
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        self.current_frame = self.current_frame.max(frame);
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
                    particle_owner,
                );
            } else {
                self.deactivate_projectile_exhausts_for(*projectile_id, particle_owner);
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
            self.deactivate(id, particle_owner);
        }
    }

    fn upsert_projectile_exhaust(
        &mut self,
        position: Vec3,
        frame: u32,
        shooter: ObjectId,
        projectile_id: ObjectId,
        exhaust_name: &str,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
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
            self.deactivate(duplicate_id, particle_owner);
        }

        let Some(existing_id) = existing_id else {
            return self.spawn_projectile_exhaust_entry(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
                particle_owner,
            );
        };

        let template_matches = self
            .systems
            .get(&existing_id)
            .is_some_and(|entry| entry.template_name == exhaust_name);
        if !template_matches {
            self.deactivate(existing_id, particle_owner);
            return self.spawn_projectile_exhaust_entry(
                position,
                frame,
                shooter,
                projectile_id,
                exhaust_name,
                particle_owner,
            );
        }

        self.update_client_position(existing_id, position, particle_owner);

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
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> u32 {
        let id = self.spawn_with_template(
            CombatParticleKind::ProjectileExhaust,
            exhaust_name.to_string(),
            position,
            frame,
            Some(shooter),
            Some(projectile_id),
        );
        self.bind_source_entry(id, particle_owner);
        id
    }

    fn deactivate_projectile_exhausts_for(
        &mut self,
        projectile_id: ObjectId,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
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
            self.deactivate(id, particle_owner);
        }
    }

    /// C++ `ObjectCreationList.cpp:962-969` attach ParticleSystem to a spawned object.
    pub fn attach_named_to_object(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        template_name: &str,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> Option<u32> {
        self.attach_named_to_object_local(
            owner,
            position,
            0.0,
            Vec3::ZERO,
            frame,
            template_name,
            CombatParticleKind::ParticleSysBone,
            None,
            particle_owner,
        )
    }

    pub fn attach_named_to_object_with_visibility(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        template_name: &str,
        is_shrouded: bool,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> Option<u32> {
        self.attach_named_to_object_local_with_visibility(
            owner,
            position,
            0.0,
            Vec3::ZERO,
            frame,
            template_name,
            CombatParticleKind::ParticleSysBone,
            None,
            is_shrouded,
            particle_owner,
        )
    }

    /// C++ create + `setPosition` (local) + `attachToObject` [+ `setSystemLifetime`].
    pub fn attach_named_to_object_local(
        &mut self,
        owner: ObjectId,
        origin: Vec3,
        yaw: f32,
        local: Vec3,
        frame: u32,
        template_name: &str,
        kind: CombatParticleKind,
        lifetime: Option<u32>,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> Option<u32> {
        self.attach_named_to_object_local_with_visibility(
            owner,
            origin,
            yaw,
            local,
            frame,
            template_name,
            kind,
            lifetime,
            true,
            particle_owner,
        )
    }

    pub fn attach_named_to_object_local_with_visibility(
        &mut self,
        owner: ObjectId,
        origin: Vec3,
        yaw: f32,
        local: Vec3,
        frame: u32,
        template_name: &str,
        kind: CombatParticleKind,
        lifetime: Option<u32>,
        is_shrouded: bool,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) -> Option<u32> {
        Some(spawn_attached_system(
            self,
            kind,
            template_name,
            owner,
            origin,
            yaw,
            local,
            frame,
            lifetime,
            0.0,
            is_shrouded,
            particle_owner,
        ))
    }

    /// C++ `W3DModelDraw::recalcBonesForClientParticleSystems` for one object.
    pub(crate) fn sync_particle_sys_bones(
        &mut self,
        frame: u32,
        owner: ObjectId,
        position: Vec3,
        bones: &[(String, String)],
        pose: BodyAutoParticlePose<'_>,
        source: Option<&crate::game_logic::NativeSlaveSource>,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ReceivingParticlePoseAccess,
    ) {
        let wanted: Vec<(String, String)> = bones
            .iter()
            .filter_map(|(bone, system)| {
                usable_particle_template_name(system)
                    .map(|system| (bone.clone(), system.to_string()))
            })
            .collect();
        let wanted_templates: HashSet<String> =
            wanted.iter().map(|(_, system)| system.clone()).collect();
        let stale: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && entry.kind == CombatParticleKind::ParticleSysBone
                    && entry.source_object == Some(owner)
                    && !wanted_templates.contains(&entry.template_name)
            })
            .map(|entry| entry.id)
            .collect();
        for id in stale {
            self.deactivate(id, particle_owner);
        }
        for (bone, system) in wanted {
            // C++ treats a missing bone name (`NONE`) as the object's origin.
            // An authored named bone is different: missing receiving pose data
            // must not invent an origin spawn or overwrite an existing binding.
            let (local, z_rot) = if bone.trim().is_empty() || bone.eq_ignore_ascii_case("none") {
                (Vec3::ZERO, 0.0)
            } else {
                let Some(source) = source else {
                    log::error!(
                        "receiving ParticleSysBone source unavailable for named bone {} on {:?}",
                        bone,
                        owner
                    );
                    continue;
                };
                match particle_sys_bone_pose_from_receiver(&bone, source, particle_owner) {
                    Ok(Some(pose)) => pose,
                    Ok(None) => {
                        log::error!(
                            "receiving ParticleSysBone named bone {} has no transform on {:?}",
                            bone,
                            owner
                        );
                        continue;
                    }
                    Err(error) => {
                        log::error!(
                            "receiving ParticleSysBone pose unavailable for named bone {} on {:?}: {error}",
                            bone, owner
                        );
                        continue;
                    }
                }
            };
            let world = rotate_yaw_host(position, pose.yaw, local);
            let existing = self.systems.values().find_map(|entry| {
                (entry.active
                    && entry.kind == CombatParticleKind::ParticleSysBone
                    && entry.source_object == Some(owner)
                    && entry.template_name == system)
                    .then_some(entry.id)
            });
            if let Some(id) = existing {
                if let Some(entry) = self.systems.get_mut(&id) {
                    entry.attach_offset = local;
                    entry.position = world;
                }
                // C++ rotateLocalTransformZ is incremental (Rotate_Z). Apply once
                // at spawn only — re-applying every sync_live_state_particles tick
                // would spin exhaust/stacks.
            } else {
                let _ = spawn_attached_system(
                    self,
                    CombatParticleKind::ParticleSysBone,
                    &system,
                    owner,
                    position,
                    pose.yaw,
                    local,
                    frame,
                    None,
                    z_rot,
                    pose.is_shrouded.unwrap_or(true),
                    particle_owner,
                );
            }
        }
    }

    /// C++ `ActiveBody::updateBodyParticleSystems` on body-state change.
    pub(crate) fn replace_body_auto_particles(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        aflame: bool,
        pose: BodyAutoParticlePose<'_>,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ReceivingParticlePoseAccess,
        random: &mut game_engine::common::random_value::RandomState,
    ) {
        let specs = body_auto_particle_specs(aflame, &self.body_auto_particle_effective);
        let locals = specs
            .iter()
            .map(|spec| match pose.source {
                Some(source) => match body_prefix_bone_locals_from_receiver(
                    source,
                    spec.prefix.as_str(),
                    MAX_BODY_PARTICLE_BONES,
                    particle_owner,
                ) {
                    Ok(locals) => locals,
                    Err(error) => {
                        log::error!("receiving body particle bones unavailable: {error}");
                        Vec::new()
                    }
                },
                None => Vec::new(),
            })
            .collect();
        self.replace_body_auto_particles_resolved(
            owner,
            position,
            frame,
            specs,
            locals,
            pose.yaw,
            pose.is_shrouded,
            particle_owner,
            random,
        );
    }

    #[cfg(test)]
    pub fn replace_body_auto_particles_with_bones(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        aflame: bool,
        prefix_bones: &[(String, Vec<Vec3>)],
        particle_owner: &mut dyn crate::game_logic::combat_particles::ReceivingParticlePoseAccess,
        random: &mut game_engine::common::random_value::RandomState,
    ) {
        let specs = body_auto_particle_specs(aflame, &self.body_auto_particle_effective);
        let locals = specs
            .iter()
            .map(|spec| {
                prefix_bones
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&spec.prefix))
                    .map(|(_, bones)| bones.iter().map(|world| *world - position).collect())
                    .unwrap_or_default()
            })
            .collect();
        self.replace_body_auto_particles_resolved(
            owner,
            position,
            frame,
            specs,
            locals,
            0.0,
            None,
            particle_owner,
            random,
        );
    }

    fn replace_body_auto_particles_resolved<P: crate::game_logic::combat_particles::ParticleBindingAccess + ?Sized>(
        &mut self,
        owner: ObjectId,
        position: Vec3,
        frame: u32,
        specs: Vec<BodyAutoParticleSpec>,
        locals: Vec<Vec<Vec3>>,
        yaw: f32,
        is_shrouded: Option<bool>,
        particle_owner: &mut P,
        random: &mut game_engine::common::random_value::RandomState,
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
            self.deactivate(id, particle_owner);
        }
        for (spec, locals) in specs.into_iter().zip(locals) {
            let Some(template) = usable_particle_template_name(spec.template.as_str()) else {
                continue;
            };
            spawn_body_systems_on_bones(
                self,
                spec.kind,
                template,
                owner,
                position,
                yaw,
                frame,
                spec.max_systems,
                &locals,
                is_shrouded.unwrap_or(true),
                particle_owner,
                random,
            );
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

    pub fn follow_attached_body_particles(&mut self, owner: ObjectId, position: Vec3, yaw: f32) {
        let ids: Vec<u32> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && matches!(
                        entry.kind,
                        CombatParticleKind::BodyFire
                            | CombatParticleKind::BodySmoke
                            | CombatParticleKind::DisableFx
                    )
                    && entry.source_object == Some(owner)
            })
            .map(|entry| entry.id)
            .collect();
        for id in ids {
            if let Some(entry) = self.systems.get_mut(&id) {
                // C++ `ParticleSystem::update`: parentXfrm * local. Do not write
                // world into leftover `setPosition` (that field is local).
                entry.position = rotate_yaw_host(position, yaw, entry.attach_offset);
            }
        }
    }

    /// HeightDie destroys systems attached to this receiving object only.
    pub(crate) fn destroy_attached_to_object(
        &mut self,
        owner: ObjectId,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        let ids: Vec<_> = self
            .systems
            .values()
            .filter(|entry| {
                entry.active
                    && match entry.kind {
                        CombatParticleKind::ParticleSysBone
                        | CombatParticleKind::BodyFire
                        | CombatParticleKind::BodySmoke
                        | CombatParticleKind::DisableFx => entry.source_object == Some(owner),
                        CombatParticleKind::ProjectileExhaust => {
                            entry.target_object == Some(owner)
                                || (entry.target_object.is_none()
                                    && entry.source_object == Some(owner))
                        }
                        _ => false,
                    }
            })
            .map(|entry| entry.id)
            .collect();
        for id in ids {
            self.deactivate(id, particle_owner);
        }
    }

    pub fn deactivate<P: crate::game_logic::combat_particles::ParticleBindingAccess + ?Sized>(
        &mut self,
        id: u32,
        particle_owner: &mut P,
    ) {
        let _ = self.try_deactivate(id, particle_owner);
    }

    /// Keep the ownership receipt until its actual client resource retires.
    /// Callers holding an attached ID must retain it when this returns false.
    pub(crate) fn try_deactivate<P: crate::game_logic::combat_particles::ParticleBindingAccess + ?Sized>(
        &mut self,
        id: u32,
        particle_owner: &mut P,
    ) -> bool {
        if let Some(binding) = self.client_bindings.get(&id) {
            if !retire_client_binding(binding, particle_owner) {
                return false;
            }
        }
        let Some(entry) = self.systems.get_mut(&id).filter(|entry| entry.active) else {
            return true;
        };
        entry.active = false;
        entry.client_system_id = None;
        self.client_bindings.remove(&id);
        true
    }
}

/// C++ `INI::parseParticleSystemTemplate` treats `None` as a null template.
/// Keep that sentinel out of the runtime registry rather than inventing a
/// system named `None`.
fn usable_particle_template_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then_some(name)
}

/// C++ `MAX_BONES` in `ActiveBody::createParticleSystems`.
const MAX_BODY_PARTICLE_BONES: usize = 16;

/// Live-host pose for FIRE/SMOKE/AFLAME prefix bone lookup.
#[derive(Clone, Copy, Debug)]
pub struct BodyAutoParticlePose<'a> {
    pub model: &'a str,
    pub scale: f32,
    pub yaw: f32,
    pub is_shrouded: Option<bool>,
    pub(crate) source: Option<&'a crate::game_logic::NativeSlaveSource>,
}

impl<'a> BodyAutoParticlePose<'a> {
    pub fn new(model: &'a str, scale: f32, yaw: f32) -> Self {
        Self {
            model,
            scale,
            yaw,
            is_shrouded: None,
            source: None,
        }
    }
    pub fn with_visibility(model: &'a str, scale: f32, yaw: f32, is_shrouded: bool) -> Self {
        Self {
            model,
            scale,
            yaw,
            is_shrouded: Some(is_shrouded),
            source: None,
        }
    }
    pub(crate) fn with_receiving_source(
        model: &'a str,
        scale: f32,
        yaw: f32,
        is_shrouded: bool,
        source: &'a crate::game_logic::NativeSlaveSource,
    ) -> Self {
        Self {
            model,
            scale,
            yaw,
            is_shrouded: Some(is_shrouded),
            source: Some(source),
        }
    }
}

struct BodyAutoParticleSpec {
    kind: CombatParticleKind,
    prefix: String,
    template: String,
    max_systems: i32,
}

/// C++ `ActiveBody::updateBodyParticleSystems` Small+Medium+Large + aflame swap.
fn body_auto_particle_specs(
    aflame: bool,
    data: &game_engine::common::ini::ini_game_data::BodyAutoParticleRules,
) -> Vec<BodyAutoParticleSpec> {
    let count_modifier = if aflame { 2i32 } else { 1i32 };
    let mut specs = vec![
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodyFire,
            prefix: data.fire_small_prefix.clone(),
            template: if aflame {
                data.fire_medium_system.clone()
            } else {
                data.fire_small_system.clone()
            },
            max_systems: data.fire_small_max.saturating_mul(count_modifier),
        },
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodyFire,
            prefix: data.fire_medium_prefix.clone(),
            template: if aflame {
                data.fire_large_system.clone()
            } else {
                data.fire_medium_system.clone()
            },
            max_systems: data.fire_medium_max.saturating_mul(count_modifier),
        },
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodyFire,
            prefix: data.fire_large_prefix.clone(),
            template: data.fire_large_system.clone(),
            max_systems: data.fire_large_max.saturating_mul(count_modifier),
        },
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodySmoke,
            prefix: data.smoke_small_prefix.clone(),
            template: if aflame {
                data.fire_small_system.clone()
            } else {
                data.smoke_small_system.clone()
            },
            max_systems: data.smoke_small_max.saturating_mul(count_modifier),
        },
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodySmoke,
            prefix: data.smoke_medium_prefix.clone(),
            template: if aflame {
                data.fire_small_system.clone()
            } else {
                data.smoke_medium_system.clone()
            },
            max_systems: data.smoke_medium_max.saturating_mul(count_modifier),
        },
        BodyAutoParticleSpec {
            kind: CombatParticleKind::BodySmoke,
            prefix: data.smoke_large_prefix.clone(),
            template: if aflame {
                data.fire_small_system.clone()
            } else {
                data.smoke_large_system.clone()
            },
            max_systems: data.smoke_large_max.saturating_mul(count_modifier),
        },
    ];
    if aflame {
        specs.push(BodyAutoParticleSpec {
            kind: CombatParticleKind::BodyFire,
            prefix: data.aflame_prefix.clone(),
            template: data.aflame_system.clone(),
            max_systems: data.aflame_max.saturating_mul(count_modifier),
        });
    }
    specs
}

fn host_local_to_cpp(local: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(local.x, local.z, local.y)
}

fn rotate_yaw_host(origin: Vec3, yaw: f32, local: Vec3) -> Vec3 {
    let (sin, cos) = yaw.sin_cos();
    Vec3::new(
        origin.x + local.x * cos - local.z * sin,
        origin.y + local.y,
        origin.z + local.x * sin + local.z * cos,
    )
}

/// C++ `W3DModelDraw::recalcBonesForClientParticleSystems` uses the receiving
/// installed draw's bone lookup. A successful lookup with zero matches is a
/// valid origin placement; absence of that receiving capability is an error.
fn particle_sys_bone_pose_from_receiver(
    bone: &str,
    source: &crate::game_logic::NativeSlaveSource,
    particle_owner: &mut dyn crate::game_logic::combat_particles::ReceivingParticlePoseAccess,
) -> Result<Option<(Vec3, f32)>, String> {
    if bone.trim().is_empty() || bone.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let transforms = particle_owner
        .current_bone_transforms(source, bone, 0, 1)?
        .ok_or_else(|| "receiving installed-draw bone capability unavailable".to_string())?;
    let Some(transform) = transforms.into_iter().next() else {
        // C++ Get_Bone_Index returns zero for an unknown name; its caller then
        // still creates the system at the object origin.
        return Ok(Some((Vec3::ZERO, 0.0)));
    };
    let translation = transform.w_axis.truncate();
    let cols = transform.to_cols_array();
    let z_rotation = cols[1].atan2(cols[0]);
    Ok(Some((
        Vec3::new(translation.x, translation.z, translation.y),
        z_rotation,
    )))
}

fn body_prefix_bone_locals_from_receiver(
    source: &crate::game_logic::NativeSlaveSource,
    prefix: &str,
    max: usize,
    particle_owner: &mut dyn crate::game_logic::combat_particles::ReceivingParticlePoseAccess,
) -> Result<Vec<Vec3>, String> {
    if prefix.is_empty() || max == 0 {
        return Ok(Vec::new());
    }
    // ActiveBody::createParticleSystems specifically calls the receiving
    // pristine-bone path; current animated HAnim transforms are not equivalent.
    let transforms = particle_owner
        .pristine_bone_transforms(source, prefix, 1, max)?
        .ok_or_else(|| "receiving pristine-bone capability unavailable".to_string())?;
    Ok(transforms
        .into_iter()
        .map(|transform| {
            let translation = transform.w_axis.truncate();
            Vec3::new(translation.x, translation.z, translation.y)
        })
        .collect())
}

/// C++ create + local `setPosition` + attach. Never an unattached xyz preset.
fn spawn_attached_system<P: crate::game_logic::combat_particles::ParticleBindingAccess + ?Sized>(
    registry: &mut CombatParticleRegistry,
    kind: CombatParticleKind,
    template: &str,
    owner: ObjectId,
    origin: Vec3,
    yaw: f32,
    local: Vec3,
    frame: u32,
    lifetime: Option<u32>,
    local_z_rot: f32,
    is_shrouded: bool,
    particle_owner: &mut P,
) -> u32 {
    let world = rotate_yaw_host(origin, yaw, local);
    let id = registry.next_id;
    registry.next_id = registry.next_id.saturating_add(1).max(1);
    registry.current_frame = registry.current_frame.max(frame);
    let mut entry = CombatParticleSystemEntry {
        id,
        kind,
        template_name: template.to_string(),
        position: world,
        source_object: Some(owner),
        target_object: None,
        spawned_frame: frame,
        active: true,
        client_system_id: None,
        fx_list_name: String::new(),
        ocl_list_name: String::new(),
        attach_offset: local,
        attachment: Some(CombatParticleAttachment {
            owner,
            local_offset: local,
            lifetime,
            local_z_rotation: local_z_rot,
        }),
    };
    if let Some(binding) = spawn_client_binding(&entry, &registry.client_bindings, particle_owner) {
        entry.client_system_id = Some(binding.id);
        registry.client_bindings.insert(id, binding);
    }
    registry.systems.insert(id, entry);
    registry.spawned_this_frame.push(id);
    id
}

/// C++ `ActiveBody::createParticleSystems` random unused-bone pick.
fn spawn_body_systems_on_bones<P: crate::game_logic::combat_particles::ParticleBindingAccess + ?Sized>(
    registry: &mut CombatParticleRegistry,
    kind: CombatParticleKind,
    template: &str,
    owner: ObjectId,
    object_origin: Vec3,
    yaw: f32,
    frame: u32,
    max_systems: i32,
    bone_locals: &[Vec3],
    is_shrouded: bool,
    particle_owner: &mut P,
    random: &mut game_engine::common::random_value::RandomState,
) {
    if max_systems <= 0 || bone_locals.is_empty() {
        return;
    }
    let num_bones = bone_locals.len();
    let target_count = usize::min(max_systems as usize, num_bones);
    let mut used = vec![false; num_bones];
    for i in 0..target_count {
        let available = target_count - i;
        // ActiveBody.cpp999-1002: pick among this operation's unused prefix
        // slots. Empty/single choices never consume the receiving stream.
        let pick = if available <= 1 {
            0
        } else {
            random.next_u32() as usize % available
        };
        let mut selected = None;
        let mut free_count = 0usize;
        for (idx, used_bone) in used.iter().enumerate() {
            if *used_bone {
                continue;
            }
            if free_count == pick {
                selected = Some(idx);
                break;
            }
            free_count += 1;
        }
        let Some(bone_index) = selected else {
            continue;
        };
        used[bone_index] = true;
        spawn_attached_system(
            registry,
            kind,
            template,
            owner,
            object_origin,
            yaw,
            bone_locals[bone_index],
            frame,
            None,
            0.0,
            is_shrouded,
            particle_owner,
        );
    }
}

/// C++ current-state `ParticleSysBone` list for a live host object.
/// Resolve current-state ParticleSysBone rows from an already admitted
/// receiving Object projection. `None` is an explicit miss; callers must not
/// select another world's process AssetManager catalog.
pub fn particle_sys_bones_for_receiving_definition(
    definition: Option<&crate::assets::ObjectDefinition>,
    _template_name: &str,
    condition_bits: u128,
) -> Vec<(String, String)> {
    definition
        .map(|definition| definition.particle_sys_bones_for_conditions(condition_bits))
        .unwrap_or_default()
}

fn standalone_particle_sys_bone_fallback(template_name: &str) -> Vec<(String, String)> {
    let mut bones = Vec::new();
    if template_name.contains("RepairVehiclesInArea") {
        bones.push((
            "NONE".to_string(),
            crate::game_logic::host_emergency_repair::EMERGENCY_REPAIR_CLOUD_PARTICLE.to_string(),
        ));
    }
    if template_name.contains("Frenzy") && template_name.contains("Marker") {
        bones.push((
            "NONE".to_string(),
            crate::game_logic::host_frenzy::FRENZY_CLOUD_PARTICLE.to_string(),
        ));
    }
    bones
}

/// Unadmitted standalone compatibility route. Live admitted sessions call
/// `particle_sys_bones_for_receiving_definition` against their own projection.
pub fn particle_sys_bones_for_template(
    template_name: &str,
    condition_bits: u128,
) -> Vec<(String, String)> {
    let definition = crate::assets::get_asset_manager().and_then(|manager| {
        manager.lock().ok().and_then(|guard| {
            guard
                .get_object_definition(template_name)
                .or_else(|| guard.resolve_object_definition(template_name, None))
                .cloned()
        })
    });
    let bones = particle_sys_bones_for_receiving_definition(
        definition.as_ref(),
        template_name,
        condition_bits,
    );
    if bones.is_empty() {
        standalone_particle_sys_bone_fallback(template_name)
    } else {
        bones
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pristine_body_still_resolves_authored_auto_particle_bones() {
        use game_engine::common::ini::ini_game_data::BodyAutoParticleRules;
        let mut rules = BodyAutoParticleRules::default();
        rules.fire_small_prefix = "FIRESMALL".into();
        rules.fire_small_system = "PristineFireWitness".into();
        rules.fire_small_max = 1;
        let mut registry = CombatParticleRegistry::new();
        registry.admit_body_auto_particle_baseline(rules);
        let owner = ObjectId(901);
        registry.replace_body_auto_particles_with_bones(
            owner,
            Vec3::ZERO,
            7,
            false,
            &[("FIRESMALL".into(), vec![Vec3::new(1.0, 0.0, 0.0)])],
            &mut (),
            &mut Default::default(),
        );
        assert!(registry
            .systems_of_kind(CombatParticleKind::BodyFire)
            .iter()
            .any(|entry| entry.source_object == Some(owner)
                && entry.template_name == "PristineFireWitness"));
        registry.clear().unwrap();
    }

    #[test]
    fn body_particle_rules_follow_each_registry_and_map_reset_uses_its_baseline() {
        use game_engine::common::ini::ini_game_data::BodyAutoParticleRules;
        let mut a_rules = BodyAutoParticleRules::default();
        a_rules.fire_small_prefix = "FIRESMALL".into();
        a_rules.fire_small_system = "WorldAFire".into();
        a_rules.fire_small_max = 1;
        let mut b_rules = BodyAutoParticleRules::default();
        b_rules.fire_small_prefix = "FIRESMALL".into();
        b_rules.fire_small_system = "WorldBFire".into();
        b_rules.fire_small_max = 2;

        let mut a = CombatParticleRegistry::new();
        let mut b = CombatParticleRegistry::new();
        a.admit_body_auto_particle_baseline(a_rules.clone());
        b.admit_body_auto_particle_baseline(b_rules.clone());
        let map_rules = a_rules
            .clone()
            .overlay_ini_text("GameData\nAutoFireParticleSmallSystem = WorldAMapFire\nEnd")
            .unwrap();
        a.apply_body_auto_particle_map_rules(map_rules);

        let a_map = body_auto_particle_specs(false, &a.body_auto_particle_effective);
        let b_live = body_auto_particle_specs(false, &b.body_auto_particle_effective);
        assert_eq!(a_map[0].template, "WorldAMapFire");
        assert_eq!(b_live[0].template, "WorldBFire");
        a.reset_body_auto_particle_map_rules();
        let a_reset = body_auto_particle_specs(false, &a.body_auto_particle_effective);
        assert_eq!(a_reset[0].template, "WorldAFire");
        assert_eq!(b_live[0].template, "WorldBFire");
    }

    #[test]
    fn body_particle_specs_preserve_empty_templates_prefixes_and_nonpositive_counts() {
        use game_engine::common::ini::ini_game_data::BodyAutoParticleRules;
        let mut rules = BodyAutoParticleRules::default();
        rules.fire_small_prefix.clear();
        rules.fire_small_system.clear();
        rules.fire_small_max = 0;
        rules.fire_medium_prefix = "AuthoredMediumBone".into();
        rules.fire_medium_system = "None".into();
        rules.fire_medium_max = -3;

        let specs = body_auto_particle_specs(false, &rules);
        assert_eq!(specs[0].prefix, "");
        assert_eq!(specs[0].template, "");
        assert_eq!(specs[0].max_systems, 0);
        assert_eq!(specs[1].prefix, "AuthoredMediumBone");
        assert_eq!(specs[1].template, "None");
        assert_eq!(specs[1].max_systems, -3);
    }

    #[test]
    fn leftover_manager_feed_swizzles_host_y_up_to_z_up() {
        // Host `(x, height, z_ground)` → leftover/C++ `(x, y_ground, z_height)`.
        let leftover = host_local_to_cpp(Vec3::new(10.0, 20.0, 30.0));
        assert!((leftover.x - 10.0).abs() < f32::EPSILON);
        assert!((leftover.y - 30.0).abs() < f32::EPSILON);
        assert!((leftover.z - 20.0).abs() < f32::EPSILON);
    }

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
    fn registry_spawn_fire_creates_muzzle_entry_only() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx(
            Vec3::new(1.0, 0.0, 1.0),
            Some(Vec3::new(50.0, 0.0, 50.0)),
            3,
            ObjectId(1),
            Some(ObjectId(2)),
        );
        // C++ Weapon.cpp:903-939: without an authored DetonationFX list the
        // impact plays nothing — no generic BulletImpact preset spawn.
        assert_eq!(ids.len(), 1);
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
                .len(),
            1
        );
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::WeaponImpact).len(),
            0
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
        loaded.restore_from_snapshot(next, snap).unwrap();
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
    fn authored_fire_and_detonation_fx_do_not_mirror_client_presets() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named(
            Vec3::ZERO,
            Some(Vec3::ONE),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "WeaponFX_GenericTankGunNoTracer",
            "WeaponFX_JetMissileDetonation",
        );
        assert_eq!(ids.len(), 2);
        let muzzle = reg.get(ids[0]).expect("muzzle");
        assert!(
            muzzle.client_system_id.is_none(),
            "FireFX is an FXList; ParticleSystems come from dispatch only"
        );
        let impact = reg.get(ids[1]).expect("impact");
        assert!(
            impact.client_system_id.is_none(),
            "DetonationFX is an FXList; ParticleSystems come from dispatch only"
        );
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
            0.0,
            0.0,
        );
        assert_eq!(ids.len(), 2);
        let muzzle = reg.systems.get(&ids[0]).expect("muzzle");
        assert_eq!(muzzle.ocl_list_name, "OCL_FireFieldSmall");
        let impact = reg.systems.get(&ids[1]).expect("impact");
        assert_eq!(impact.fx_list_name, "FX_Detonate");
        assert_eq!(impact.ocl_list_name, "OCL_PoisonFieldMedium");
    }

    #[test]
    fn fire_fx_dispatch_accepts_weapon_speed() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named_ocl(
            Vec3::ZERO,
            Some(Vec3::new(30.0, 0.0, 0.0)),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "WeaponFX_HumveeMachineGun",
            "",
            "",
            "",
            600.0,
            0.0,
        );
        assert!(!ids.is_empty());
    }

    #[test]
    fn fire_fx_dispatch_threads_primary_damage_radius() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named_ocl(
            Vec3::ZERO,
            Some(Vec3::new(30.0, 0.0, 0.0)),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "WeaponFX_TomahawkMissile",
            "WeaponFX_TomahawkMissileDetonation",
            "",
            "",
            0.0,
            25.0,
        );
        assert_eq!(ids.len(), 2);
    }
    #[test]
    fn empty_detonation_fx_spawns_no_generic_impact_preset() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named_ocl(
            Vec3::ZERO,
            Some(Vec3::ONE),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "FX_Muzzle",
            "",
            "",
            "",
            0.0,
            0.0,
        );
        // C++ Weapon.cpp:903-939 plays nothing at the impact without an
        // authored DetonationFX list — never a generic BulletImpact preset.
        assert_eq!(ids.len(), 1, "only the muzzle entry may spawn");
        assert!(!reg
            .systems
            .values()
            .any(|entry| entry.kind == CombatParticleKind::WeaponImpact),);
    }

    #[test]
    fn one_shot_systems_deactivate_once_system_lifetime_elapses() {
        let mut reg = CombatParticleRegistry::new();
        let explosion = reg.spawn(
            CombatParticleKind::DeathExplosion,
            Vec3::ZERO,
            5,
            Some(ObjectId(1)),
            None,
        );
        let smoke = reg.spawn(
            CombatParticleKind::DeathSmoke,
            Vec3::ZERO,
            5,
            Some(ObjectId(1)),
            None,
        );

        // Registry frame still at 0: lifetime has not elapsed.
        reg.clear_frame_events();
        assert!(reg.get(explosion).expect("explosion").active);

        // frame - spawned_frame reaches the 2-frame MediumExplosion lifetime.
        reg.sync_projectile_exhausts(7, &[]);
        reg.clear_frame_events();
        assert!(!reg.get(explosion).expect("explosion").active);
        // SmokePlume residual lives 600 frames — untouched at 7.
        assert!(reg.get(smoke).expect("smoke").active);

        reg.sync_projectile_exhausts(605, &[]);
        reg.clear_frame_events();
        assert!(!reg.get(smoke).expect("smoke").active);
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
        use game_engine::common::ini::ini_game_data::BodyAutoParticleRules;
        let mut reg = CombatParticleRegistry::new();
        let mut rules = BodyAutoParticleRules::default();
        rules.fire_small_prefix = "FIRESMALL".into();
        rules.fire_small_system = "FireSmall".into();
        rules.fire_small_max = 1;
        rules.fire_medium_prefix = "FIREMEDIUM".into();
        rules.fire_medium_system = "FireMedium".into();
        rules.fire_medium_max = 1;
        rules.fire_large_prefix = "FIRELARGE".into();
        rules.fire_large_system = "FireLarge".into();
        rules.fire_large_max = 1;
        rules.smoke_small_prefix = "SMOKESMALL".into();
        rules.smoke_small_system = "SmokeSmall".into();
        rules.smoke_small_max = 1;
        rules.smoke_medium_prefix = "SMOKEMEDIUM".into();
        rules.smoke_medium_system = "SmokeMedium".into();
        rules.smoke_medium_max = 1;
        rules.smoke_large_prefix = "SMOKELARGE".into();
        rules.smoke_large_system = "SmokeLarge".into();
        rules.smoke_large_max = 1;
        rules.aflame_prefix = "AFLAME".into();
        rules.aflame_system = "Aflame".into();
        rules.aflame_max = 1;
        reg.admit_body_auto_particle_baseline(rules);
        let owner = ObjectId(11);
        let pos = Vec3::new(2.0, 0.0, 3.0);
        reg.sync_particle_sys_bones(
            4,
            owner,
            pos,
            &[("None".to_string(), "DieselSmoke".to_string())],
            BodyAutoParticlePose::new("", 1.0, 0.0),
            None,
            &mut (),
        );
        let bones = reg.systems_of_kind(CombatParticleKind::ParticleSysBone);
        assert_eq!(bones.len(), 1);
        assert_eq!(bones[0].template_name, "DieselSmoke");
        assert_eq!(bones[0].source_object, Some(owner));

        let fire_bone = pos + Vec3::new(5.0, 2.0, 1.0);
        let smoke_bone = pos + Vec3::new(-4.0, 3.0, 2.0);
        let large_bone = pos + Vec3::new(7.0, 4.0, -1.0);
        let aflame_bone = pos + Vec3::new(0.0, 5.0, 0.0);
        reg.replace_body_auto_particles_with_bones(
            owner,
            pos,
            5,
            false,
            &[
                ("FIRESMALL".to_string(), vec![fire_bone]),
                ("SMOKESMALL".to_string(), vec![smoke_bone]),
                ("FIRELARGE".to_string(), vec![large_bone]),
            ],
            &mut (),
            &mut Default::default(),
        );
        assert!(reg.has_body_particles(owner));
        let fires: Vec<_> = reg
            .systems_of_kind(CombatParticleKind::BodyFire)
            .into_iter()
            .cloned()
            .collect();
        let smokes: Vec<_> = reg
            .systems_of_kind(CombatParticleKind::BodySmoke)
            .into_iter()
            .cloned()
            .collect();
        assert!(!fires.is_empty());
        assert!(!smokes.is_empty());
        assert!(
            fires
                .iter()
                .any(|e| (e.position - fire_bone).length() < 0.01),
            "small fire must sit on FIRESMALL bone, not origin"
        );
        assert!(
            fires
                .iter()
                .any(|e| (e.position - large_bone).length() < 0.01),
            "large fire tier must spawn when FIRELARGE bones exist"
        );
        assert!(
            smokes
                .iter()
                .any(|e| (e.position - smoke_bone).length() < 0.01),
            "smoke must sit on SMOKESMALL bone, not origin"
        );
        assert!(fires.iter().all(|e| (e.position - pos).length() > 0.5));

        reg.replace_body_auto_particles_with_bones(
            owner,
            pos,
            6,
            true,
            &[
                (
                    "FIRESMALL".to_string(),
                    vec![fire_bone, fire_bone + Vec3::X],
                ),
                ("SMOKESMALL".to_string(), vec![smoke_bone]),
                ("AFLAME".to_string(), vec![aflame_bone]),
            ],
            &mut (),
            &mut Default::default(),
        );
        let aflame_fires: Vec<_> = reg
            .systems_of_kind(CombatParticleKind::BodyFire)
            .into_iter()
            .cloned()
            .collect();
        assert!(
            aflame_fires
                .iter()
                .any(|e| (e.position - aflame_bone).length() < 0.01),
            "aflame prefix systems must attach at AFLAME bones"
        );
        assert!(
            aflame_fires.len() >= 3,
            "aflame should keep FIRESMALL bones plus AFLAME-prefix systems"
        );
        let aflame_smokes: Vec<_> = reg
            .systems_of_kind(CombatParticleKind::BodySmoke)
            .into_iter()
            .cloned()
            .collect();
        assert!(
            aflame_smokes
                .iter()
                .any(|e| e.template_name == "FireSmall" || e.template_name.contains("FireSmall")),
            "aflame swaps smoke templates to small fire"
        );

        let moved = pos + Vec3::new(10.0, 0.0, 0.0);
        reg.follow_attached_body_particles(owner, moved, 0.0);
        let followed = reg
            .systems_of_kind(CombatParticleKind::BodyFire)
            .into_iter()
            .find(|e| (e.attach_offset - (aflame_bone - pos)).length() < 0.01)
            .expect("aflame entry");
        assert!((followed.position - (moved + (aflame_bone - pos))).length() < 0.01);

        reg.replace_body_auto_particles(
            owner,
            pos,
            7,
            false,
            BodyAutoParticlePose::new("", 1.0, 0.0),
            &mut (),
            &mut Default::default(),
        );
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
        let repair =
            particle_sys_bones_for_template("RepairVehiclesInArea_InvisibleMarker_Level1", 0);
        assert!(repair.iter().any(|(_, system)| system == "RepairCloud"));
        let frenzy = particle_sys_bones_for_template("Frenzy_InvisibleMarker", 0);
        assert!(frenzy.iter().any(|(_, system)| system == "FrenzyCloud"));
    }

    #[test]
    fn particle_sys_bone_sits_on_local_offset_not_origin() {
        let mut reg = CombatParticleRegistry::new();
        let owner = ObjectId(21);
        let origin = Vec3::new(10.0, 0.0, 4.0);
        let local = Vec3::new(3.0, 2.0, 1.0);
        let id = reg
            .attach_named_to_object_local(
                owner,
                origin,
                0.0,
                local,
                1,
                "DieselSmoke",
                CombatParticleKind::ParticleSysBone,
                None,
            )
            .expect("bone system");
        let entry = reg.get(id).expect("entry");
        assert!((entry.attach_offset - local).length() < 0.01);
        assert!((entry.position - (origin + local)).length() < 0.01);
        assert!((entry.position - origin).length() > 0.5);
    }

    #[test]
    fn disable_fx_is_not_culled_as_particle_sys_bone() {
        let mut reg = CombatParticleRegistry::new();
        let victim = ObjectId(9);
        let origin = Vec3::new(5.0, 0.0, 5.0);
        let local = Vec3::new(2.0, 0.0, 1.0);
        let id = reg
            .attach_named_to_object_local(
                victim,
                origin,
                0.0,
                local,
                3,
                "DisabledEffectBinaryShower0",
                CombatParticleKind::DisableFx,
                Some(120),
            )
            .expect("disable fx");
        assert!(reg.get(id).expect("entry").active);
        reg.sync_particle_sys_bones(
            4,
            victim,
            origin,
            &[],
            BodyAutoParticlePose::new("", 1.0, 0.0),
            None,
            &mut (),
        );
        assert!(
            reg.get(id).expect("kept").active,
            "DisableFX must not be culled as a stale ParticleSysBone"
        );
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::ParticleSysBone)
                .len(),
            0
        );
    }

    #[test]
    fn attached_fire_follows_parent_times_local_yaw() {
        use game_engine::common::ini::ini_game_data::BodyAutoParticleRules;
        let mut reg = CombatParticleRegistry::new();
        let mut rules = BodyAutoParticleRules::default();
        rules.fire_small_prefix = "FIRESMALL".into();
        rules.fire_small_system = "FireSmall".into();
        rules.fire_small_max = 1;
        reg.admit_body_auto_particle_baseline(rules);
        let owner = ObjectId(3);
        let origin = Vec3::new(0.0, 0.0, 0.0);
        let fire_bone = Vec3::new(5.0, 2.0, 0.0);
        reg.replace_body_auto_particles_with_bones(
            owner,
            origin,
            1,
            false,
            &[("FIRESMALL".to_string(), vec![fire_bone])],
            &mut (),
            &mut Default::default(),
        );
        let yaw = std::f32::consts::FRAC_PI_2;
        reg.follow_attached_body_particles(owner, origin, yaw);
        let fire = reg
            .systems_of_kind(CombatParticleKind::BodyFire)
            .into_iter()
            .next()
            .expect("fire");
        let expected = rotate_yaw_host(origin, yaw, fire_bone);
        assert!(
            (fire.position - expected).length() < 0.01,
            "fire must rotate with hull, got {:?} expected {:?}",
            fire.position,
            expected
        );
        assert!(
            (fire.position - fire_bone).length() > 0.5,
            "world translation follow would stay at {:?}",
            fire_bone
        );
    }
}
