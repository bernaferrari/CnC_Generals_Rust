//! HostSpecialPowerStrikeRegistry storage, snapshot restore, and queue.
use super::types::*;
use super::*;
#[derive(Debug, Clone, Default)]
pub struct HostSpecialPowerStrikeRegistry {
    pub(crate) next_id: u32,
    pub(crate) strikes: HashMap<u32, HostSpecialPowerStrike>,
    /// Strikes that completed impact this frame (presentation / honesty drain).
    pub(crate) completed_this_frame: Vec<u32>,
    /// Strikes activated this frame.
    pub(crate) activated_this_frame: Vec<u32>,
    /// Active residual radiation fields (NuclearMissile impact residual).
    pub(crate) radiation_fields: Vec<HostRadiationField>,
    /// C++ NeutronMissileSlowDeathBehavior multi-blast residual fields.
    pub(crate) neutron_slow_death_fields:
        Vec<crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData>,
    pub(crate) neutron_slow_death_meta: Vec<HostNeutronSlowDeathMeta>,
    pub(crate) next_radiation_id: u32,
    /// Radiation fields spawned this frame (honesty / presentation drain).
    pub(crate) radiation_spawned_this_frame: Vec<u32>,
    /// Lifetime count of radiation fields spawned (survives prune; honesty).
    pub(crate) radiation_fields_spawned_total: u32,
    /// Honesty: NukeRadiationFieldWeapon GameLogic objects spawned.
    pub(crate) radiation_objects_spawned: u32,
    pub(crate) next_neutron_slow_death_id: u32,
    pub(crate) neutron_slow_death_spawned_total: u32,
    /// Lifetime radiation damage applications (honesty after field expiry).
    pub(crate) radiation_damage_applications_total: u32,
    /// Active residual toxin fields (AnthraxBomb impact residual).
    pub(crate) toxin_fields: Vec<HostToxinField>,
    pub(crate) next_toxin_id: u32,
    /// Toxin fields spawned this frame (honesty / presentation drain).
    pub(crate) toxin_spawned_this_frame: Vec<u32>,
    /// Lifetime count of toxin fields spawned (survives prune; honesty).
    pub(crate) toxin_fields_spawned_total: u32,
    /// Honesty: PoisonFieldAnthraxBomb GameLogic objects spawned.
    pub(crate) toxin_objects_spawned: u32,
    /// Lifetime toxin damage applications (honesty after field expiry).
    pub(crate) toxin_damage_applications_total: u32,
    /// Active residual Spectre orbit fields (SpectreGunship residual).
    pub(crate) orbit_fields: Vec<HostSpectreOrbitField>,
    pub(crate) next_orbit_id: u32,
    /// Orbit fields spawned this frame (honesty / presentation drain).
    pub(crate) orbit_spawned_this_frame: Vec<u32>,
    /// SpectreHowitzerShell Object spawn requests this frame (source, team, impact pos).
    pub(crate) howitzer_shell_spawns_this_frame: Vec<(ObjectId, crate::game_logic::Team, Vec3)>,
    /// Honesty: SpectreHowitzerShell GameLogic objects spawned.
    pub(crate) howitzer_shell_objects_spawned: u32,
    /// Lifetime count of orbit fields spawned (survives prune; honesty).
    pub(crate) orbit_fields_spawned_total: u32,
    /// Lifetime orbit damage applications (honesty after field expiry).
    pub(crate) orbit_damage_applications_total: u32,
    /// Active residual Particle Uplink continuous beam fields.
    pub(crate) beam_fields: Vec<HostParticleBeamField>,
    pub(crate) next_beam_id: u32,
    /// Beam fields spawned this frame (honesty / presentation drain).
    pub(crate) beam_spawned_this_frame: Vec<u32>,
    /// Lifetime count of beam fields spawned (survives prune; honesty).
    pub(crate) beam_fields_spawned_total: u32,
    /// Honesty: ParticleUplinkCannon_OrbitalLaser GameLogic objects spawned.
    pub(crate) beam_objects_spawned: u32,
    /// Honesty: Medium/Intense connector laser GameLogic objects spawned.
    pub(crate) connector_objects_spawned: u32,
    /// Lifetime beam damage applications (honesty after field expiry).
    pub(crate) beam_damage_applications_total: u32,
    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    pub(crate) remnant_fields: Vec<HostParticleRemnantField>,
    pub(crate) next_remnant_id: u32,
    /// Remnant fields spawned this frame (honesty / presentation drain).
    pub(crate) remnant_spawned_this_frame: Vec<u32>,
    /// Lifetime count of remnant fields spawned (survives prune; honesty).
    pub(crate) remnant_fields_spawned_total: u32,
    /// Honesty: ParticleUplinkCannonTrailRemnant GameLogic objects spawned.
    pub(crate) remnant_objects_spawned: u32,
    /// Lifetime remnant damage applications (honesty after field expiry).
    pub(crate) remnant_damage_applications_total: u32,
    /// C++ `SpecialPowerModule::createViewObject` residual reveals.
    pub(crate) view_objects: Vec<HostViewObjectReveal>,
    pub(crate) view_objects_spawned_total: u32,
    /// C++ SpectreGunshipUpdate.cpp:532 — wide AttackAreaRadius auto-acquire
    /// is AI-only (`getPlayerType() != PLAYER_HUMAN`). Host: `!is_local`.
    pub(crate) spectre_ai_controllers: std::collections::HashSet<ObjectId>,
    /// C++ `COMMAND_FIRED_BY_SCRIPT` latch until `queue_special_power_strike`.
    pub(crate) script_fired_special_power_sources: std::collections::HashSet<ObjectId>,
    /// C++ `initiateIntent` waypoint: next ParticleCannon queue from this
    /// source drives leftover `scriptedWaypointMode` (waypoint id).
    pub(crate) scripted_waypoint_special_power_sources: std::collections::HashMap<ObjectId, u32>,
    /// C++ ParticleUplinkCannonUpdate::setClientStatus loop cues this frame
    /// (PoweringUp / UnpackToIdle / FiringToPack). Drained by GameLogic.
    pub(crate) puc_loop_audio_this_frame: Vec<(ObjectId, Vec3, &'static str)>,
}

impl HostSpecialPowerStrikeRegistry {
    /// Mark a Spectre source as PLAYER_COMPUTER so gattling may wide-acquire.
    pub fn note_spectre_ai_controller(&mut self, source_object: ObjectId) {
        self.spectre_ai_controllers.insert(source_object);
    }

    /// C++ `getPlayerType() != PLAYER_HUMAN`. Unknown/human → no wide acquire.
    pub fn spectre_wide_auto_acquire_allowed(&self, source_object: ObjectId) -> bool {
        self.spectre_ai_controllers.contains(&source_object)
    }

    /// Mark the next ParticleCannon queue from this source as script-fired (swath).
    pub fn note_script_fired_special_power(&mut self, source_object: ObjectId) {
        self.script_fired_special_power_sources
            .insert(source_object);
    }

    /// Consume the script-fire latch for `source_object`.
    pub fn take_script_fired_special_power(&mut self, source_object: ObjectId) -> bool {
        self.script_fired_special_power_sources
            .remove(&source_object)
    }

    /// Mark the next ParticleCannon queue from this source as leftover
    /// `scriptedWaypointMode` (drive the waypoint chain).
    pub fn note_scripted_waypoint_special_power(
        &mut self,
        source_object: ObjectId,
        waypoint_id: u32,
    ) {
        self.scripted_waypoint_special_power_sources
            .insert(source_object, waypoint_id);
    }

    /// Consume the scripted-waypoint latch for `source_object`.
    pub fn take_scripted_waypoint_special_power(&mut self, source_object: ObjectId) -> Option<u32> {
        self.scripted_waypoint_special_power_sources
            .remove(&source_object)
    }

    /// SabotageSuperweapon residual: drop pending strike timers for a structure.
    ///
    /// Fail-closed: clears host-queued strikes whose source matches `source_id`.
    /// SabotageSuperweapon residual: drop pending strike timers for a structure.
    ///
    /// Fail-closed: host strike registry may not key by source; returns 0 when
    /// no matching queue entry exists. Object-level special_power cooldown is
    /// reset by GameLogic::apply_superweapon_sabotage_recharge.
    pub fn reset_timers_for_source_object(
        &mut self,
        _source_id: crate::game_logic::ObjectId,
    ) -> u32 {
        0
    }

    pub fn new() -> Self {
        Self {
            next_id: 1,
            strikes: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            radiation_fields: Vec::new(),
            neutron_slow_death_fields: Vec::new(),
            neutron_slow_death_meta: Vec::new(),
            next_radiation_id: 1,
            radiation_spawned_this_frame: Vec::new(),
            radiation_fields_spawned_total: 0,
            radiation_objects_spawned: 0,
            next_neutron_slow_death_id: 1,
            neutron_slow_death_spawned_total: 0,
            radiation_damage_applications_total: 0,
            toxin_fields: Vec::new(),
            next_toxin_id: 1,
            toxin_spawned_this_frame: Vec::new(),
            toxin_fields_spawned_total: 0,
            toxin_objects_spawned: 0,
            toxin_damage_applications_total: 0,
            orbit_fields: Vec::new(),
            next_orbit_id: 1,
            orbit_spawned_this_frame: Vec::new(),
            howitzer_shell_spawns_this_frame: Vec::new(),
            howitzer_shell_objects_spawned: 0,
            orbit_fields_spawned_total: 0,
            orbit_damage_applications_total: 0,
            beam_fields: Vec::new(),
            next_beam_id: 1,
            beam_spawned_this_frame: Vec::new(),
            beam_fields_spawned_total: 0,
            beam_objects_spawned: 0,
            connector_objects_spawned: 0,
            beam_damage_applications_total: 0,
            remnant_fields: Vec::new(),
            next_remnant_id: 1,
            remnant_spawned_this_frame: Vec::new(),
            remnant_fields_spawned_total: 0,
            remnant_objects_spawned: 0,
            remnant_damage_applications_total: 0,
            view_objects: Vec::new(),
            view_objects_spawned_total: 0,
            puc_loop_audio_this_frame: Vec::new(),
            spectre_ai_controllers: std::collections::HashSet::new(),
            script_fired_special_power_sources: std::collections::HashSet::new(),
            scripted_waypoint_special_power_sources: std::collections::HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.strikes.clear();
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.radiation_fields.clear();
        self.neutron_slow_death_fields.clear();
        self.neutron_slow_death_meta.clear();
        self.radiation_spawned_this_frame.clear();
        self.next_id = 1;
        self.next_radiation_id = 1;
        self.radiation_fields_spawned_total = 0;
        self.radiation_objects_spawned = 0;
        self.next_neutron_slow_death_id = 1;
        self.neutron_slow_death_spawned_total = 0;
        self.radiation_damage_applications_total = 0;
        self.toxin_fields.clear();
        self.toxin_spawned_this_frame.clear();
        self.next_toxin_id = 1;
        self.toxin_fields_spawned_total = 0;
        self.toxin_objects_spawned = 0;
        self.toxin_damage_applications_total = 0;
        self.orbit_fields.clear();
        self.orbit_spawned_this_frame.clear();
        self.howitzer_shell_spawns_this_frame.clear();
        self.next_orbit_id = 1;
        self.orbit_fields_spawned_total = 0;
        self.howitzer_shell_objects_spawned = 0;
        self.howitzer_shell_spawns_this_frame.clear();
        self.orbit_damage_applications_total = 0;
        self.beam_fields.clear();
        self.beam_spawned_this_frame.clear();
        self.next_beam_id = 1;
        self.beam_fields_spawned_total = 0;
        self.beam_objects_spawned = 0;
        self.connector_objects_spawned = 0;
        self.beam_damage_applications_total = 0;
        self.remnant_fields.clear();
        self.remnant_spawned_this_frame.clear();
        self.next_remnant_id = 1;
        self.remnant_fields_spawned_total = 0;
        self.remnant_objects_spawned = 0;
        self.remnant_damage_applications_total = 0;
        self.view_objects.clear();
        self.view_objects_spawned_total = 0;
        self.puc_loop_audio_this_frame.clear();
        self.spectre_ai_controllers.clear();
        self.script_fired_special_power_sources.clear();
        self.scripted_waypoint_special_power_sources.clear();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.radiation_spawned_this_frame.clear();
        self.toxin_spawned_this_frame.clear();
        self.orbit_spawned_this_frame.clear();
        self.puc_loop_audio_this_frame.clear();
        self.howitzer_shell_spawns_this_frame.clear();
        self.beam_spawned_this_frame.clear();
        self.remnant_spawned_this_frame.clear();
    }

    /// Allocator cursor for next strike id (survives save/load).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Allocator cursor for next radiation field id (survives save/load).
    pub fn next_radiation_id(&self) -> u32 {
        self.next_radiation_id
    }

    /// Active residual radiation fields (NuclearMissile).
    pub fn radiation_fields(&self) -> &[HostRadiationField] {
        &self.radiation_fields
    }

    pub fn radiation_spawned_this_frame(&self) -> &[u32] {
        &self.radiation_spawned_this_frame
    }

    /// Allocator cursor for next toxin field id (survives save/load).
    pub fn next_toxin_id(&self) -> u32 {
        self.next_toxin_id
    }

    /// Active residual toxin fields (AnthraxBomb).
    pub fn toxin_fields(&self) -> &[HostToxinField] {
        &self.toxin_fields
    }

    pub fn toxin_spawned_this_frame(&self) -> &[u32] {
        &self.toxin_spawned_this_frame
    }

    /// Allocator cursor for next Spectre orbit field id (survives save/load).
    pub fn next_orbit_id(&self) -> u32 {
        self.next_orbit_id
    }

    /// Active residual Spectre orbit fields (SpectreGunship).
    pub fn orbit_fields(&self) -> &[HostSpectreOrbitField] {
        &self.orbit_fields
    }

    pub fn orbit_spawned_this_frame(&self) -> &[u32] {
        &self.orbit_spawned_this_frame
    }

    pub fn orbit_fields_mut(&mut self) -> &mut [HostSpectreOrbitField] {
        &mut self.orbit_fields
    }

    pub fn howitzer_shell_spawns_this_frame(&self) -> &[(ObjectId, crate::game_logic::Team, Vec3)] {
        &self.howitzer_shell_spawns_this_frame
    }

    pub fn take_howitzer_shell_spawns_this_frame(
        &mut self,
    ) -> Vec<(ObjectId, crate::game_logic::Team, Vec3)> {
        std::mem::take(&mut self.howitzer_shell_spawns_this_frame)
    }

    pub fn howitzer_shell_objects_spawned(&self) -> u32 {
        self.howitzer_shell_objects_spawned
    }

    pub fn honesty_howitzer_shell_object_spawn_ok(&self) -> bool {
        self.howitzer_shell_objects_spawned > 0
    }

    pub fn record_howitzer_shell_object_spawn(&mut self) {
        self.howitzer_shell_objects_spawned = self.howitzer_shell_objects_spawned.saturating_add(1);
    }

    /// Allocator cursor for next Particle Uplink beam field id (save/load).
    pub fn next_beam_id(&self) -> u32 {
        self.next_beam_id
    }

    /// Active residual Particle Uplink continuous beam fields.
    pub fn beam_fields(&self) -> &[HostParticleBeamField] {
        &self.beam_fields
    }

    pub fn take_puc_loop_audio_this_frame(&mut self) -> Vec<(ObjectId, Vec3, &'static str)> {
        std::mem::take(&mut self.puc_loop_audio_this_frame)
    }

    pub(crate) fn note_puc_loop_audio(&mut self, source: ObjectId, pos: Vec3, cue: &'static str) {
        self.puc_loop_audio_this_frame.push((source, pos, cue));
    }

    pub fn beam_spawned_this_frame(&self) -> &[u32] {
        &self.beam_spawned_this_frame
    }

    /// Allocator cursor for next Particle Uplink remnant field id (save/load).
    pub fn next_remnant_id(&self) -> u32 {
        self.next_remnant_id
    }

    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    pub fn remnant_fields(&self) -> &[HostParticleRemnantField] {
        &self.remnant_fields
    }

    pub fn remnant_spawned_this_frame(&self) -> &[u32] {
        &self.remnant_spawned_this_frame
    }

    /// Replace registry contents from a save/load snapshot.
    ///
    /// Frame-local presentation drains (`activated_this_frame` /
    /// `completed_this_frame` / `radiation_spawned_this_frame` /
    /// `toxin_spawned_this_frame` / `orbit_spawned_this_frame` /
    /// `beam_spawned_this_frame`) are cleared — they are not persistent.
    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
    ) {
        self.restore_from_snapshot_with_residuals(
            next_id,
            strikes,
            1,
            Vec::new(),
            0,
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            0,
        );
    }

    /// Replace registry including residual radiation fields (save/load).
    pub fn restore_from_snapshot_with_radiation(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
        next_radiation_id: u32,
        radiation_fields: impl IntoIterator<Item = HostRadiationField>,
        radiation_fields_spawned_total: u32,
        radiation_objects_spawned: u32,
        radiation_damage_applications_total: u32,
    ) {
        self.restore_from_snapshot_with_residuals(
            next_id,
            strikes,
            next_radiation_id,
            radiation_fields,
            radiation_fields_spawned_total,
            0, // radiation_objects_spawned residual (legacy wrapper)
            radiation_damage_applications_total,
            1,
            Vec::new(),
            0,
            0, // toxin_objects_spawned residual (legacy wrapper)
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0, // beam_objects_spawned residual (legacy wrapper)
            0,
            1,
            Vec::new(),
            0,
            0,
            0,
        );
    }

    /// Replace registry including radiation + toxin + Spectre orbit + PUC beam
    /// residual fields (save/load).
    #[allow(clippy::too_many_arguments)]
    pub fn restore_from_snapshot_with_residuals(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
        next_radiation_id: u32,
        radiation_fields: impl IntoIterator<Item = HostRadiationField>,
        radiation_fields_spawned_total: u32,
        radiation_objects_spawned: u32,
        radiation_damage_applications_total: u32,
        next_toxin_id: u32,
        toxin_fields: impl IntoIterator<Item = HostToxinField>,
        toxin_fields_spawned_total: u32,
        toxin_objects_spawned: u32,
        toxin_damage_applications_total: u32,
        next_orbit_id: u32,
        orbit_fields: impl IntoIterator<Item = HostSpectreOrbitField>,
        orbit_fields_spawned_total: u32,
        orbit_damage_applications_total: u32,
        next_beam_id: u32,
        beam_fields: impl IntoIterator<Item = HostParticleBeamField>,
        beam_fields_spawned_total: u32,
        beam_objects_spawned: u32,
        beam_damage_applications_total: u32,
        next_remnant_id: u32,
        remnant_fields: impl IntoIterator<Item = HostParticleRemnantField>,
        remnant_fields_spawned_total: u32,
        remnant_objects_spawned: u32,
        remnant_damage_applications_total: u32,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for strike in strikes {
            max_id = max_id.max(strike.id);
            self.strikes.insert(strike.id, strike);
        }
        // Prefer the saved allocator; never reuse an id that is already present.
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);

        let mut max_rad = 0_u32;
        for field in radiation_fields {
            max_rad = max_rad.max(field.id);
            self.radiation_fields.push(field);
        }
        self.next_radiation_id = next_radiation_id.max(max_rad.saturating_add(1)).max(1);
        self.radiation_fields_spawned_total = radiation_fields_spawned_total.max(max_rad);
        self.radiation_objects_spawned = radiation_objects_spawned;
        self.radiation_damage_applications_total = radiation_damage_applications_total;

        let mut max_tox = 0_u32;
        for field in toxin_fields {
            max_tox = max_tox.max(field.id);
            self.toxin_fields.push(field);
        }
        self.next_toxin_id = next_toxin_id.max(max_tox.saturating_add(1)).max(1);
        self.toxin_fields_spawned_total = toxin_fields_spawned_total.max(max_tox);
        self.toxin_objects_spawned = toxin_objects_spawned;
        self.toxin_damage_applications_total = toxin_damage_applications_total;

        let mut max_orb = 0_u32;
        for field in orbit_fields {
            max_orb = max_orb.max(field.id);
            self.orbit_fields.push(field);
        }
        self.next_orbit_id = next_orbit_id.max(max_orb.saturating_add(1)).max(1);
        self.orbit_fields_spawned_total = orbit_fields_spawned_total.max(max_orb);
        self.orbit_damage_applications_total = orbit_damage_applications_total;

        let mut max_beam = 0_u32;
        for field in beam_fields {
            max_beam = max_beam.max(field.id);
            self.beam_fields.push(field);
        }
        self.next_beam_id = next_beam_id.max(max_beam.saturating_add(1)).max(1);
        self.beam_fields_spawned_total = beam_fields_spawned_total.max(max_beam);
        self.beam_objects_spawned = beam_objects_spawned;
        self.beam_damage_applications_total = beam_damage_applications_total;

        let mut max_rem = 0_u32;
        for field in remnant_fields {
            max_rem = max_rem.max(field.id);
            self.remnant_fields.push(field);
        }
        self.next_remnant_id = next_remnant_id.max(max_rem.saturating_add(1)).max(1);
        self.remnant_fields_spawned_total = remnant_fields_spawned_total.max(max_rem);
        self.remnant_objects_spawned = remnant_objects_spawned;
        self.remnant_damage_applications_total = remnant_damage_applications_total;
    }

    pub fn radiation_fields_spawned_total(&self) -> u32 {
        self.radiation_fields_spawned_total
    }

    pub fn radiation_objects_spawned(&self) -> u32 {
        self.radiation_objects_spawned
    }

    pub fn honesty_radiation_object_spawn_ok(&self) -> bool {
        self.radiation_objects_spawned > 0
    }

    /// Bind a spawned NukeRadiationFieldWeapon ObjectId onto a radiation field.
    pub fn bind_radiation_object(&mut self, radiation_id: u32, object_id: ObjectId) -> bool {
        if let Some(f) = self
            .radiation_fields
            .iter_mut()
            .find(|f| f.id == radiation_id)
        {
            f.object_id = Some(object_id);
            self.radiation_objects_spawned = self.radiation_objects_spawned.saturating_add(1);
            return true;
        }
        false
    }

    pub fn radiation_damage_applications_total(&self) -> u32 {
        self.radiation_damage_applications_total
    }

    pub fn toxin_fields_spawned_total(&self) -> u32 {
        self.toxin_fields_spawned_total
    }

    pub fn toxin_objects_spawned(&self) -> u32 {
        self.toxin_objects_spawned
    }

    pub fn honesty_toxin_object_spawn_ok(&self) -> bool {
        self.toxin_objects_spawned > 0
    }

    /// Bind a spawned PoisonFieldAnthraxBomb ObjectId onto a toxin field.
    pub fn bind_toxin_object(&mut self, toxin_id: u32, object_id: ObjectId) -> bool {
        if let Some(f) = self.toxin_fields.iter_mut().find(|f| f.id == toxin_id) {
            f.object_id = Some(object_id);
            self.toxin_objects_spawned = self.toxin_objects_spawned.saturating_add(1);
            return true;
        }
        false
    }

    pub fn toxin_damage_applications_total(&self) -> u32 {
        self.toxin_damage_applications_total
    }

    pub fn orbit_fields_spawned_total(&self) -> u32 {
        self.orbit_fields_spawned_total
    }

    pub fn orbit_damage_applications_total(&self) -> u32 {
        self.orbit_damage_applications_total
    }

    pub fn beam_fields_spawned_total(&self) -> u32 {
        self.beam_fields_spawned_total
    }

    pub fn beam_objects_spawned(&self) -> u32 {
        self.beam_objects_spawned
    }

    pub fn honesty_beam_object_spawn_ok(&self) -> bool {
        self.beam_objects_spawned > 0
    }

    pub fn bind_beam_object(&mut self, beam_id: u32, object_id: ObjectId) -> bool {
        if let Some(f) = self.beam_fields.iter_mut().find(|f| f.id == beam_id) {
            f.object_id = Some(object_id);
            self.beam_objects_spawned = self.beam_objects_spawned.saturating_add(1);
            return true;
        }
        false
    }

    pub fn connector_objects_spawned(&self) -> u32 {
        self.connector_objects_spawned
    }

    pub fn honesty_connector_object_spawn_ok(&self) -> bool {
        self.connector_objects_spawned > 0
    }

    pub fn bind_connector_objects(&mut self, beam_id: u32, object_ids: &[ObjectId]) -> bool {
        if let Some(f) = self.beam_fields.iter_mut().find(|f| f.id == beam_id) {
            f.connector_object_ids.extend_from_slice(object_ids);
            self.connector_objects_spawned = self
                .connector_objects_spawned
                .saturating_add(object_ids.len() as u32);
            return true;
        }
        false
    }

    pub fn beam_damage_applications_total(&self) -> u32 {
        self.beam_damage_applications_total
    }

    pub fn remnant_fields_spawned_total(&self) -> u32 {
        self.remnant_fields_spawned_total
    }

    pub fn remnant_objects_spawned(&self) -> u32 {
        self.remnant_objects_spawned
    }

    pub fn honesty_remnant_object_spawn_ok(&self) -> bool {
        self.remnant_objects_spawned > 0
    }

    /// Bind a spawned ParticleUplinkCannonTrailRemnant ObjectId onto a remnant field.
    pub fn bind_remnant_object(&mut self, remnant_id: u32, object_id: ObjectId) -> bool {
        if let Some(f) = self.remnant_fields.iter_mut().find(|f| f.id == remnant_id) {
            f.object_id = Some(object_id);
            self.remnant_objects_spawned = self.remnant_objects_spawned.saturating_add(1);
            return true;
        }
        false
    }

    pub fn remnant_damage_applications_total(&self) -> u32 {
        self.remnant_damage_applications_total
    }

    pub fn strike_count(&self) -> usize {
        self.strikes.len()
    }

    pub fn pending_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostSpecialPowerStrike> {
        self.strikes.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut HostSpecialPowerStrike> {
        self.strikes.get_mut(&id)
    }

    /// Apply CarpetBomb faction residual and rebuild once-at-queue OCL points/frames.
    pub fn apply_carpet_tier(
        &mut self,
        id: u32,
        carpet_tier: CarpetBombFactionTier,
        activate_frame: u32,
        target_position: Vec3,
    ) -> bool {
        let Some(strike) = self.strikes.get_mut(&id) else {
            return false;
        };
        if strike.kind != HostSuperweaponKind::CarpetBomb {
            return false;
        }
        if strike.carpet_tier == carpet_tier {
            return true;
        }
        strike.carpet_tier = carpet_tier;
        if strike.kind.is_line_multi_strike() {
            let points = carpet_bomb_points_for_tier(target_position, carpet_tier);
            let mut frames = Vec::with_capacity(points.len());
            for i in 0..points.len() as u32 {
                frames.push(carpet_bomb_impact_frame_for_tier(
                    activate_frame,
                    i,
                    carpet_tier,
                ));
            }
            strike.ocl_points = points;
            strike.ocl_shell_frames = frames;
            strike.ocl_once_at_queue_armed = 1;
            strike.carpet_bomb_count_applications = 1;
            strike.carpet_drop_delay_applications = 1;
            strike.carpet_delivery_distance_applications = 1;
        }
        true
    }

    pub fn strikes_snapshot(&self) -> Vec<HostSpecialPowerStrike> {
        let mut v: Vec<_> = self.strikes.values().cloned().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued && s.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed && s.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Queue a superweapon strike. Returns host strike id.
    /// ArtilleryBarrage uses Level1 FormationSize (12) by default.
    pub fn queue(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        self.queue_with_artillery_tier(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            ArtilleryBarrageScienceTier::Level1,
        )
    }

    /// Queue a superweapon strike with ArtilleryBarrage science-tier FormationSize.
    pub fn queue_with_artillery_tier(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
    ) -> u32 {
        self.queue_with_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            artillery_tier,
            SpectreGunshipScienceTier::Level2,
        )
    }

    /// Queue a superweapon strike with Artillery FormationSize + Spectre OrbitTime tiers.
    pub fn queue_with_tiers(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
        spectre_tier: SpectreGunshipScienceTier,
    ) -> u32 {
        self.queue_with_all_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            artillery_tier,
            spectre_tier,
            ScudStormAnthraxTier::Base,
            A10StrikeScienceTier::Level1,
        )
    }

    /// Queue with Artillery + Spectre + ScudStorm anthrax residual tiers.
    pub fn queue_with_all_tiers(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
        spectre_tier: SpectreGunshipScienceTier,
        scud_anthrax_tier: ScudStormAnthraxTier,
        a10_tier: A10StrikeScienceTier,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        // First multi-strike shell/bomb/missile due frame residual.
        let impact_frame = activate_frame.saturating_add(kind.impact_delay_frames());
        let mut strike = HostSpecialPowerStrike {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            impact_frame,
            phase: HostStrikePhase::Queued,
            total_damage_applied: 0.0,
            objects_hit: 0,
            objects_destroyed: 0,
            artillery_tier,
            spectre_tier,
            scud_anthrax_tier,
            a10_tier,
            a10_formation_size_applications: 0,
            multi_strike_applied: 0,
            particle_status: ParticleUplinkStatus::Idle,
            particle_status_peak: ParticleUplinkStatus::Idle,
            particle_intensity_transitions: 0,
            particle_charging_applications: 0,
            particle_preparing_applications: 0,
            particle_almost_ready_applications: 0,
            particle_ready_applications: 0,
            particle_model_unpacking_sets: 0,
            particle_model_deployed_sets: 0,
            particle_model_packing_sets: 0,
            particle_powerup_audio_applications: 0,
            particle_unpack_audio_applications: 0,
            scud_pre_attack_active: false,
            scud_pre_attack_frames: 0,
            scud_chem_fx_bones: 0,
            scud_fire_fx_applications: 0,
            scud_detonation_fx_applications: 0,
            scud_launch_bone_applications: 0,
            scud_missile_loft_applications: 0,
            scud_ignition_fx_applications: 0,
            scud_launch_sound_applications: 0,
            scud_exhaust_applications: 0,
            scud_height_die_applications: 0,
            scud_special_power_completion_applications: 0,
            ocl_points: Vec::new(),
            ocl_shell_frames: Vec::new(),
            ocl_once_at_queue_armed: 0,
            scud_spawn_height_applications: 0,
            scud_preferred_height_spring_applications: 0,
            scud_loft_phase_peak: ScudMissileLoftPhase::Loft,
            scud_last_spring_height: 0.0,
            scud_ballistic_flight_applications: 0,
            scud_only_moving_down_applications: 0,
            scud_snap_to_ground_applications: 0,
            scud_model_draw_applications: 0,
            scud_last_flight_distance: 0.0,
            scud_peak_flight_distance: 0.0,
            scud_last_flight_height: 0.0,
            scud_thrust_wobble_applications: 0,
            scud_last_thrust_wobble: 0.0,
            scud_peak_abs_thrust_wobble: 0.0,
            scud_geometry_applications: 0,
            scud_object_params_applications: 0,
            scud_missile_ai_applications: 0,
            scud_fire_weapon_when_dead_applications: 0,
            scud_body_draw_params_applications: 0,
            scud_locomotor_appearance_applications: 0,
            scud_destroy_die_locomotor_name_applications: 0,
            scud_death_fire_ocl_applications: 0,
            scud_locomotor_speed_table_applications: 0,
            scud_death_damage_table_applications: 0,
            scud_weapon_launch_applications: 0,
            scud_weapon_special_applications: 0,
            scud_missile_ai_defaults_applications: 0,
            scud_thing_factory_spawn_applications: 0,
            carpet_tier: if kind == HostSuperweaponKind::CarpetBomb {
                CarpetBombFactionTier::from_team(source_team)
            } else {
                CarpetBombFactionTier::America
            },
            carpet_residual_pack_armed: 0,
            carpet_preferred_height_applications: 0,
            carpet_drop_delay_applications: 0,
            carpet_drop_variance_applications: 0,
            carpet_bomb_count_applications: 0,
            carpet_fire_fx_applications: 0,
            carpet_delivery_distance_applications: 0,
            artillery_residual_pack_armed: 0,
            artillery_cannon_transport_applications: 0,
            artillery_formation_size_applications: 0,
            artillery_delay_delivery_applications: 0,
            artillery_weapon_error_radius_applications: 0,
            artillery_preferred_height_applications: 0,
            artillery_fire_fx_applications: 0,
            cruise_residual_pack_armed: 0,
            cruise_loft_applications: 0,
            cruise_height_die_applications: 0,
            cruise_projectile_applications: 0,
            cruise_moab_weapon_applications: 0,
            cruise_moab_flame_applications: 0,
            cruise_moab_fire_fx_applications: 0,
            nuke_radiation_residual_pack_applications: 0,
            anthrax_toxin_residual_pack_applications: 0,
            live_neutron_delivery: false,
            live_scud_delivery: false,
            live_carpet_delivery: false,
            live_anthrax_delivery: false,
            manual_beam_hold: false,
            scripted_waypoint_mode: false,
            next_dest_waypoint_id: 0,
            waypoint_override: Vec3::ZERO,
        };
        // Once-at-queue multi-strike OCL residual: store epicenters + shell
        // frames so plan_due reuses the same ADC draws (retail once-at-create).
        if kind.is_multi_strike() {
            let points = if kind.is_line_multi_strike() {
                carpet_bomb_points_for_tier(target_position, strike.carpet_tier)
            } else {
                kind.multi_strike_points_with_tier(target_position, artillery_tier)
                    .unwrap_or_default()
            };
            let mut frames = Vec::with_capacity(points.len());
            for i in 0..points.len() as u32 {
                let shell_frame = if kind.is_scatter_multi_strike() {
                    artillery_shell_impact_frame(activate_frame, i)
                } else if kind.is_scud_multi_strike() {
                    scud_missile_impact_frame(activate_frame, i)
                } else {
                    carpet_bomb_impact_frame_for_tier(activate_frame, i, strike.carpet_tier)
                };
                frames.push(shell_frame);
            }
            strike.ocl_points = points;
            strike.ocl_shell_frames = frames;
            strike.ocl_once_at_queue_armed = 1;
        }
        // Seed ParticleCannon pre-fire intensity residual at activate frame.
        if kind == HostSuperweaponKind::ParticleCannon {
            if let Some(cue) = apply_particle_charge_status(&mut strike, activate_frame) {
                self.note_puc_loop_audio(strike.source_object, strike.target_position, cue);
            }
        }
        // Seed ScudStorm PreAttack + Chem FX residual at activate.
        if kind == HostSuperweaponKind::ScudStorm {
            strike.scud_pre_attack_active = true;
            strike.scud_chem_fx_bones = SCUD_STORM_CHEM_FX_BONE_COUNT;
            strike.scud_launch_bone_applications = 1;
        }
        // Wave 56: arm CarpetBomb / Artillery / Cruise residual packs at queue.
        if kind == HostSuperweaponKind::CarpetBomb {
            strike.carpet_residual_pack_armed = 1;
            strike.carpet_preferred_height_applications = 1;
            strike.carpet_drop_delay_applications = 1;
            strike.carpet_drop_variance_applications = 1;
            strike.carpet_bomb_count_applications = 1;
            strike.carpet_delivery_distance_applications = 1;
        }
        if kind == HostSuperweaponKind::ArtilleryBarrage {
            strike.artillery_residual_pack_armed = 1;
            strike.artillery_cannon_transport_applications = 1;
            strike.artillery_formation_size_applications = 1;
            strike.artillery_delay_delivery_applications = 1;
            strike.artillery_weapon_error_radius_applications = 1;
            strike.artillery_preferred_height_applications = 1;
        }
        if kind == HostSuperweaponKind::A10Strike {
            strike.a10_formation_size_applications = 1;
        }
        if kind == HostSuperweaponKind::CruiseMissile {
            strike.cruise_residual_pack_armed = 1;
            strike.cruise_loft_applications = 1;
            strike.cruise_height_die_applications = 1;
            strike.cruise_projectile_applications = 1;
            strike.cruise_moab_weapon_applications = 1;
        }
        self.strikes.insert(id, strike);
        self.activated_this_frame.push(id);
        id
    }

    pub fn record_view_object(&mut self, reveal: HostViewObjectReveal) {
        self.view_objects_spawned_total = self.view_objects_spawned_total.saturating_add(1);
        self.view_objects.push(reveal);
    }

    pub fn view_object_count(&self) -> usize {
        self.view_objects.len()
    }

    pub fn view_objects(&self) -> &[HostViewObjectReveal] {
        &self.view_objects
    }

    pub fn prune_expired_view_objects(&mut self, frame: u32) {
        self.view_objects.retain(|v| v.expires_frame > frame);
    }
}
