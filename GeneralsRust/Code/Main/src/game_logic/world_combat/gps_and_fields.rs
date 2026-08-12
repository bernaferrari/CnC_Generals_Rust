//! Host combat `impl GameLogic` — `gps_and_fields`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Activate GPS Scrambler residual: GrantStealth to ally vehicles/infantry in radius.
    ///
    /// Matches retail SuperweaponGPSScrambler → GPSScrambler_InvisibleMarker:
    /// - FinalRadius residual 100 (RadiusCursorRadius / GrantStealth FinalRadius)
    /// - KindOf VEHICLE | INFANTRY, same-team residual
    /// - receiveGrant → STEALTHED + clear DETECTED
    /// - Skips bomb-truck disguise residual by name (C++ canDisguise skip)
    ///
    /// Fail-closed: not full OCL marker grow-radius scan / StealthUpdate framesGranted
    /// / particle / flashAsSelected drawable path.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_gps_scrambler(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_gps_scrambler::{
            in_gps_scrambler_radius_2d, is_gps_scrambler_disguise_name,
            is_legal_gps_scrambler_target, HostGpsScrambler, GPS_SCRAMBLER_ACTIVATE_AUDIO,
            GPS_SCRAMBLER_INVISIBLE_MARKER, GPS_SCRAMBLER_START_RADIUS, HOST_GPS_SCRAMBLER_RADIUS,
        };

        let frame = self.frame;
        let center = (location.x, location.z);

        let caster_team = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .unwrap_or_else(|| match player_id {
                0 => Team::USA,
                1 => Team::China,
                2 => Team::GLA,
                _ => Team::Neutral,
            });

        let candidates: Vec<(ObjectId, bool, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                // Residual: never grant to the invisible marker/caster building itself
                // when it is a structure (command center). Units at caster pos still ok.
                let pos = obj.get_position();
                // GrantStealthBehavior StartRadius residual (grow expands later).
                if !in_gps_scrambler_radius_2d(center, (pos.x, pos.z), GPS_SCRAMBLER_START_RADIUS) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_infantry = obj.is_kind_of(KindOf::Infantry);
                let same_team = obj.team == caster_team;
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let is_disguise = is_gps_scrambler_disguise_name(&obj.template_name);
                Some((
                    *id,
                    is_vehicle,
                    is_infantry,
                    same_team,
                    under_construction,
                    is_disguise,
                ))
            })
            .collect();

        let mut grants: u32 = 0;
        for (id, is_vehicle, is_infantry, same_team, under_construction, is_disguise) in candidates
        {
            if !is_legal_gps_scrambler_target(
                is_vehicle,
                is_infantry,
                true,
                same_team,
                under_construction,
                is_disguise,
            ) {
                continue;
            }
            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let was_stealthed = target.is_effectively_stealthed();
            target.apply_grant_stealth();
            // Count new grants and refreshes as residual grant events for honesty.
            if !was_stealthed || target.is_effectively_stealthed() {
                grants = grants.saturating_add(1);
            }
        }

        // C++ OCL SUPERWEAPON_GPSScrambler → GPSScrambler_InvisibleMarker residual.
        let marker_id = self.spawn_gps_scrambler_marker(caster_team, location);

        let entry_id = self.gps_scramblers.alloc_id();
        self.gps_scramblers.record_activation(HostGpsScrambler {
            id: entry_id,
            player_id,
            location,
            radius: GPS_SCRAMBLER_START_RADIUS,
            activate_frame: frame,
            caster_id,
            grants,
            grow_index: 0,
            growing: true,
            marker_id,
        });

        // C++ SuperweaponLaunched GPS Scrambler EVA residual.
        self.try_eva_special_launched_misc(caster_team, "gps");

        self.queue_audio_event(
            AudioEventRequest::new(GPS_SCRAMBLER_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            location,
            frame,
            caster_id,
            None,
        );

        true
    }

    /// Host China ECM Tank / jammer residual: jam enemy weapons in radius.
    ///
    /// Retail inspiration:
    /// - ECMTankVehicleDisabler (SUBDUAL_VEHICLE → DISABLED_SUBDUED cannot fire)
    /// - ECMTankMissileJammer FireWeaponUpdate pulse (PrimaryDamageRadius=150,
    ///   RadiusDamageAffects = ENEMIES NEUTRALS)
    ///
    /// Fail-closed: continuous aura sets `weapons_jammed` (not full subdual damage
    /// accumulate/heal, not laser stream, not missile projectile scatter).
    pub fn update_ecm_jam_field(&mut self) {
        use crate::game_logic::host_ecm_jam::{
            in_ecm_jam_radius_2d, is_ecm_hostile_team, is_ecm_jammer, is_legal_ecm_jam_target,
            HOST_ECM_JAM_RADIUS,
        };
        use std::collections::HashSet;

        // Snapshot jammers: alive ECM tank / frequency jammer residual sources.
        let jammers: Vec<(ObjectId, Team, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_ecm_jammer(&obj.template_name) {
                    return None;
                }
                // Under construction / unmanned / hacked jammers do not emit.
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if obj.status.disabled_unmanned
                    || obj.status.disabled_hacked
                    || obj.status.disabled_emp
                {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z))
            })
            .collect();

        if jammers.is_empty() {
            // Clear residual jam when no jammers remain.
            for obj in self.objects.values_mut() {
                if obj.status.weapons_jammed {
                    obj.set_weapons_jammed(false);
                }
            }
            return;
        }

        // Snapshot armed non-structure candidates.
        let candidates: Vec<(ObjectId, Team, f32, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.is_kind_of(KindOf::Structure) {
                    return None;
                }
                if obj.status.under_construction {
                    return None;
                }
                let has_weapon = obj.weapon.is_some() || obj.secondary_weapon.is_some();
                if !has_weapon {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z, has_weapon))
            })
            .collect();

        let mut covered: HashSet<ObjectId> = HashSet::new();
        // Jammer → target links for ECMDisableStream laser residual.
        let mut jam_links: Vec<(ObjectId, ObjectId)> = Vec::new();
        for (jammer_id, jammer_team, jx, jz) in &jammers {
            let jammer_neutral = *jammer_team == Team::Neutral;
            for (target_id, target_team, tx, tz, has_weapon) in &candidates {
                let same_team = *jammer_team == *target_team;
                let target_neutral = *target_team == Team::Neutral;
                let enemy_or_neutral =
                    is_ecm_hostile_team(jammer_neutral, same_team, target_neutral);
                if !is_legal_ecm_jam_target(
                    false,
                    true,
                    enemy_or_neutral,
                    *jammer_id == *target_id,
                    false,
                    *has_weapon,
                ) {
                    continue;
                }
                if !in_ecm_jam_radius_2d((*jx, *jz), (*tx, *tz), HOST_ECM_JAM_RADIUS) {
                    continue;
                }
                covered.insert(*target_id);
                jam_links.push((*jammer_id, *target_id));
            }
        }

        let mut jam_ticks: u32 = 0;
        for target_id in &covered {
            let Some(target) = self.objects.get_mut(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            if !target.status.weapons_jammed {
                target.set_weapons_jammed(true);
                jam_ticks = jam_ticks.saturating_add(1);
            } else {
                // Already jammed — keep flag set (coverage refresh).
                target.set_status_weapons_jammed(true);
            }
        }

        // Clear jam on units no longer covered by any jammer.
        for (id, obj) in self.objects.iter_mut() {
            if covered.contains(id) {
                continue;
            }
            if obj.status.weapons_jammed {
                obj.set_weapons_jammed(false);
            }
        }

        for _ in 0..jam_ticks {
            self.record_ecm_residual_jam();
        }

        // C++ LaserName ECMDisableStream residual (VehicleDisabler WEAPONA01 bone).
        // Cadence residual: DelayBetweenShots 100ms → 3f (ExclusiveWeaponDelay fail-closed).
        {
            use crate::game_logic::host_ecm_jam::{
                ECM_DISABLE_STREAM_BONE, ECM_DISABLE_STREAM_LASER,
                ECM_VEHICLE_DISABLER_DELAY_FRAMES, ECM_VEHICLE_DISABLER_FIRE_SOUND,
            };
            use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;

            let pulse = self.frame % ECM_VEHICLE_DISABLER_DELAY_FRAMES.max(1) == 0;
            if pulse && !jam_links.is_empty() {
                // Dedup links (same pair once per pulse).
                jam_links.sort_by_key(|(a, b)| (a.0, b.0));
                jam_links.dedup();
                let mut audio_pos = None;
                for (from_id, to_id) in jam_links {
                    let Some(from_obj) = self.objects.get(&from_id) else {
                        continue;
                    };
                    let Some(to_obj) = self.objects.get(&to_id) else {
                        continue;
                    };
                    if !from_obj.is_alive() || !to_obj.is_alive() {
                        continue;
                    }
                    let from = from_obj.get_position();
                    let to = to_obj.get_position();
                    // Residual WEAPONA01 bone height.
                    let from_bone = glam::Vec3::new(from.x, from.y + 8.0, from.z);
                    let to_aim = glam::Vec3::new(to.x, to.y + 5.0, to.z);
                    let _ = self.spawn_weapon_laser_beam_object(
                        ECM_DISABLE_STREAM_LASER,
                        from_id,
                        Some(to_id),
                        from_bone,
                        to_aim,
                    );
                    self.weapon_lasers.push(ResidualWeaponLaser::with_bone(
                        ECM_DISABLE_STREAM_LASER,
                        ECM_DISABLE_STREAM_BONE,
                        from_id,
                        Some(to_id),
                        (from_bone.x, from_bone.y, from_bone.z),
                        (to_aim.x, to_aim.y, to_aim.z),
                        self.frame,
                    ));
                    self.ecm_laser_beams_spawned = self.ecm_laser_beams_spawned.saturating_add(1);
                    if audio_pos.is_none() {
                        audio_pos = Some(from);
                    }
                }
                if let Some(pos) = audio_pos {
                    self.queue_audio_event(
                        AudioEventRequest::new(ECM_VEHICLE_DISABLER_FIRE_SOUND)
                            .with_position(pos)
                            .with_priority(130),
                    );
                }
            }
        }
    }

    /// Host America Microwave Tank residual: DISABLE_SUBDUED on structures being cooked.
    ///
    /// C++ MicrowaveTankBuildingDisabler (SUBDUAL_BUILDING → DISABLED_SUBDUED when
    /// subdual >= max health). Residual: continuous while microwave is attacking a
    /// structure within AttackRange 200; structure is fully disabled (production stops).
    /// Fail-closed: not full subdual accumulate/heal, not laser stream drawable,
    /// not MicrowaveTankEmitterWeapon infantry MICROWAVE field.

    /// C++ ECMTankMissileJammer residual: jam in-flight missiles (scatter + subdual dmg).
    pub fn update_ecm_missile_jam(&mut self) {
        use crate::game_logic::host_ecm_jam::{
            ecm_missile_scatter_offset, in_ecm_jam_radius_2d, is_ecm_jam_projectile_flags,
            is_ecm_jammer, ECM_MISSILE_JAMMER_PRIMARY_DAMAGE, ECM_MISSILE_JAM_MAX_PER_PULSE,
            HOST_ECM_JAM_RADIUS,
        };

        let frame = self.frame;
        let jammers: Vec<(ObjectId, Team, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_ecm_jammer(&obj.template_name) {
                    return None;
                }
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if obj.status.disabled_unmanned
                    || obj.status.disabled_hacked
                    || obj.status.disabled_emp
                {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z))
            })
            .collect();
        if jammers.is_empty() {
            return;
        }

        let missiles: Vec<(ObjectId, Team, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.ecm_missile_jammed {
                    return None;
                }
                let proj = obj.is_kind_of(KindOf::Projectile);
                if !is_ecm_jam_projectile_flags(proj, &obj.template_name, false) {
                    return None;
                }
                let in_flight = proj
                    || obj.raptor_missile_projectile
                    || obj.mig_missile_projectile
                    || obj.tomahawk_missile_projectile
                    || obj.scud_launcher_missile_projectile
                    || obj.rocket_buggy_missile_projectile
                    || obj.rpg_trooper_missile_projectile
                    || obj.tank_hunter_missile_projectile
                    || obj.missile_defender_missile_projectile
                    || obj.scorpion_missile_projectile
                    || obj.technical_rpg_missile_projectile
                    || obj.stealth_jet_missile_projectile
                    || obj.humvee_tow_projectile
                    || obj.usa_tank_shell_projectile
                    || obj.battlemaster_shell_projectile
                    || obj.overlord_shell_projectile
                    || obj.marauder_shell_projectile
                    || obj.fire_base_shell_projectile
                    || obj.technical_cannon_shell_projectile
                    || obj.nuke_cannon_shell_projectile
                    || obj.neutron_cannon_shell_projectile
                    || obj.inferno_shell_projectile
                    || obj.scorpion_shell_projectile;
                if !in_flight {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z))
            })
            .collect();

        let mut jammed_ids: Vec<(ObjectId, ObjectId, f32, f32)> = Vec::new();
        for (jammer_id, jammer_team, jx, jz) in &jammers {
            let mut count = 0u32;
            for (mid, mteam, mx, mz) in &missiles {
                if count >= ECM_MISSILE_JAM_MAX_PER_PULSE {
                    break;
                }
                if *mteam == *jammer_team {
                    continue;
                }
                if !in_ecm_jam_radius_2d((*jx, *jz), (*mx, *mz), HOST_ECM_JAM_RADIUS) {
                    continue;
                }
                // Avoid double-queue same missile.
                if jammed_ids.iter().any(|(id, _, _, _)| *id == *mid) {
                    continue;
                }
                jammed_ids.push((*mid, *jammer_id, *mx, *mz));
                count = count.saturating_add(1);
            }
        }

        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        for (mid, jammer_id, mx, mz) in jammed_ids {
            let seed = mid.0.wrapping_add(jammer_id.0).wrapping_add(frame);
            let (sx, sz) = ecm_missile_scatter_offset(seed);
            let new_aim = [mx + sx, 0.0, mz + sz];
            let mut team = None;
            if let Some(o) = self.objects.get_mut(&mid) {
                if o.ecm_missile_jammed {
                    continue;
                }
                o.ecm_missile_jammed = true;
                team = Some(o.team);
                let killed = o.take_damage_from(ECM_MISSILE_JAMMER_PRIMARY_DAMAGE, Some(jammer_id));
                // Deflect aim residual (C++ projectile loses lock and scatters).
                if o.raptor_missile_aim.is_some() {
                    o.raptor_missile_aim = Some(new_aim);
                    o.raptor_missile_intended = None;
                }
                if o.mig_missile_aim.is_some() {
                    o.mig_missile_aim = Some(new_aim);
                    o.mig_missile_intended = None;
                }
                if o.tomahawk_missile_aim.is_some() {
                    o.tomahawk_missile_aim = Some(new_aim);
                }
                if o.scud_launcher_missile_aim.is_some() {
                    o.scud_launcher_missile_aim = Some(new_aim);
                }
                if o.rocket_buggy_missile_aim.is_some() {
                    o.rocket_buggy_missile_aim = Some(new_aim);
                    o.rocket_buggy_missile_intended = None;
                }
                if o.rpg_trooper_missile_aim.is_some() {
                    o.rpg_trooper_missile_aim = Some(new_aim);
                    o.rpg_trooper_missile_intended = None;
                }
                if o.tank_hunter_missile_aim.is_some() {
                    o.tank_hunter_missile_aim = Some(new_aim);
                    o.tank_hunter_missile_intended = None;
                }
                if o.missile_defender_missile_aim.is_some() {
                    o.missile_defender_missile_aim = Some(new_aim);
                    o.missile_defender_missile_intended = None;
                }
                if o.scorpion_missile_aim.is_some() {
                    o.scorpion_missile_aim = Some(new_aim);
                    o.scorpion_missile_intended = None;
                }
                if o.technical_rpg_missile_aim.is_some() {
                    o.technical_rpg_missile_aim = Some(new_aim);
                    o.technical_rpg_missile_intended = None;
                }
                if o.stealth_jet_missile_aim.is_some() {
                    o.stealth_jet_missile_aim = Some(new_aim);
                    o.stealth_jet_missile_intended = None;
                }
                if o.humvee_tow_aim.is_some() {
                    o.humvee_tow_aim = Some(new_aim);
                    o.humvee_tow_intended = None;
                }
                if o.usa_tank_shell_aim.is_some() {
                    o.usa_tank_shell_aim = Some(new_aim);
                    o.usa_tank_shell_intended = None;
                }
                if o.battlemaster_shell_aim.is_some() {
                    o.battlemaster_shell_aim = Some(new_aim);
                    o.battlemaster_shell_intended = None;
                }
                if o.fire_base_shell_aim.is_some() {
                    o.fire_base_shell_aim = Some(new_aim);
                    o.fire_base_shell_intended = None;
                }
                if o.technical_cannon_shell_aim.is_some() {
                    o.technical_cannon_shell_aim = Some(new_aim);
                    o.technical_cannon_shell_intended = None;
                }
                if o.inferno_shell_aim.is_some() {
                    o.inferno_shell_aim = Some(new_aim);
                    o.inferno_shell_intended = None;
                }
                if killed {
                    destroy_ids.push((mid, team));
                }
            }
            self.ecm_missiles_jammed = self.ecm_missiles_jammed.saturating_add(1);
        }
        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_ecm_missile_jam_ok(&self) -> bool {
        self.ecm_missiles_jammed > 0
    }

    pub fn update_microwave_disable(&mut self) {
        use crate::game_logic::host_microwave::{
            in_microwave_range_2d, is_legal_microwave_disable_target, is_microwave_hostile_team,
            is_microwave_tank, should_microwave_disable, HOST_MICROWAVE_DISABLE_RANGE,
            MICROWAVE_DISABLE_AUDIO,
        };
        use std::collections::HashSet;

        // Snapshot microwave tanks that are actively attacking.
        let cookers: Vec<(ObjectId, Team, ObjectId, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_microwave_tank(&obj.template_name) {
                    return None;
                }
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if obj.status.disabled_unmanned
                    || obj.status.disabled_hacked
                    || obj.status.disabled_emp
                    || obj.status.disabled_subdued
                {
                    return None;
                }
                if !obj.status.attacking {
                    return None;
                }
                let target_id = obj.target?;
                let pos = obj.get_position();
                Some((*id, obj.team, target_id, pos.x, pos.z))
            })
            .collect();

        let mut covered: HashSet<ObjectId> = HashSet::new();
        let mut first_grant_pos: Option<glam::Vec3> = None;

        for (_cooker_id, cooker_team, target_id, cx, cz) in &cookers {
            let Some(target) = self.objects.get(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let is_structure =
                target.is_kind_of(KindOf::Structure) || target.object_type == ObjectType::Building;
            let same_team = *cooker_team == target.team;
            let target_neutral = target.team == Team::Neutral;
            let cooker_neutral = *cooker_team == Team::Neutral;
            let enemy_or_neutral =
                is_microwave_hostile_team(cooker_neutral, same_team, target_neutral);
            if !is_legal_microwave_disable_target(
                is_structure,
                true,
                enemy_or_neutral,
                target.status.under_construction,
            ) {
                continue;
            }
            let tpos = target.get_position();
            if !in_microwave_range_2d((*cx, *cz), (tpos.x, tpos.z), HOST_MICROWAVE_DISABLE_RANGE) {
                continue;
            }
            if !should_microwave_disable(true, true, true, true, true, true) {
                continue;
            }
            if first_grant_pos.is_none() && !target.status.disabled_subdued {
                first_grant_pos = Some(tpos);
            }
            covered.insert(*target_id);
        }

        let mut new_grants = 0u32;
        let mut refresh_ticks = 0u32;
        for target_id in &covered {
            let Some(target) = self.objects.get_mut(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            if !target.status.disabled_subdued {
                target.set_disabled_subdued(true);
                new_grants = new_grants.saturating_add(1);
            } else {
                // Already cooked — keep flag set (coverage refresh).
                target.set_status_disabled_subdued(true);
                refresh_ticks = refresh_ticks.saturating_add(1);
            }
        }

        // Clear subdued on structures no longer cooked by any microwave.
        for (id, obj) in self.objects.iter_mut() {
            if covered.contains(id) {
                continue;
            }
            if obj.status.disabled_subdued {
                obj.set_disabled_subdued(false);
            }
        }

        for _ in 0..new_grants {
            self.microwaves.record_disable_grant();
            self.microwaves.record_disable_weapon_pulse();
        }
        for _ in 0..refresh_ticks {
            self.microwaves.record_disable_refresh();
            self.microwaves.record_disable_weapon_pulse();
        }
        self.microwaves.set_currently_disabled(covered.len() as u32);

        // C++ LaserName MicrowaveDisableStream residual attach (bone WEAPON02).
        // Spawn short-lived beam Things + presentation ResidualWeaponLaser per cook link.
        {
            use crate::game_logic::host_microwave::{
                HOST_MICROWAVE_LASER_BONE, HOST_MICROWAVE_LASER_NAME,
            };
            use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;

            let mut laser_links: Vec<(ObjectId, ObjectId, glam::Vec3, glam::Vec3)> = Vec::new();
            for (cooker_id, _team, target_id, _cx, _cz) in &cookers {
                if !covered.contains(target_id) {
                    continue;
                }
                let Some(cooker) = self.objects.get(cooker_id) else {
                    continue;
                };
                let Some(target) = self.objects.get(target_id) else {
                    continue;
                };
                if !cooker.is_alive() || !target.is_alive() {
                    continue;
                }
                laser_links.push((
                    *cooker_id,
                    *target_id,
                    cooker.get_position(),
                    target.get_position(),
                ));
            }
            for (from_id, to_id, from, to) in laser_links {
                // Raise beam origin slightly for residual WEAPON02 bone height.
                let from_bone = glam::Vec3::new(from.x, from.y + 8.0, from.z);
                let to_aim = glam::Vec3::new(to.x, to.y + 5.0, to.z);
                let _ = self.spawn_weapon_laser_beam_object(
                    HOST_MICROWAVE_LASER_NAME,
                    from_id,
                    Some(to_id),
                    from_bone,
                    to_aim,
                );
                self.weapon_lasers.push(ResidualWeaponLaser::with_bone(
                    HOST_MICROWAVE_LASER_NAME,
                    HOST_MICROWAVE_LASER_BONE,
                    from_id,
                    Some(to_id),
                    (from_bone.x, from_bone.y, from_bone.z),
                    (to_aim.x, to_aim.y, to_aim.z),
                    self.frame,
                ));
                self.microwaves.record_laser_beam();
            }
        }

        if new_grants > 0 {
            if let Some(pos) = first_grant_pos {
                self.queue_audio_event(
                    AudioEventRequest::new(MICROWAVE_DISABLE_AUDIO)
                        .with_position(pos)
                        .with_priority(140),
                );
            }
        }
    }

    /// C++ MicrowaveTankEmitterWeapon residual: MICROWAVE damage field around tank.
    ///
    /// Retail: PrimaryDamage **8**, radius **100**, Delay **250**ms, DamageDealtAtSelfPosition,
    /// RadiusDamageAffects ENEMIES NOT_AIRBORNE. Fail-closed: no ally/neutral cook, no airborne.
    pub fn update_microwave_emitter_field(&mut self) {
        use crate::game_logic::host_microwave::{
            in_microwave_range_2d, is_legal_microwave_emitter_target, is_microwave_tank,
            microwave_emitter_damage_at, HOST_MICROWAVE_EMITTER_DELAY_FRAMES,
            HOST_MICROWAVE_EMITTER_FX, HOST_MICROWAVE_EMITTER_RADIUS, MICROWAVE_DISABLE_AUDIO,
        };

        let frame = self.frame;
        // Pulse cadence residual (DelayBetweenShots 250ms → 8f).
        if frame % HOST_MICROWAVE_EMITTER_DELAY_FRAMES.max(1) != 0 {
            return;
        }

        let emitters: Vec<(ObjectId, Team, f32, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_microwave_tank(&obj.template_name) {
                    return None;
                }
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if obj.status.disabled_unmanned
                    || obj.status.disabled_hacked
                    || obj.status.disabled_emp
                    || obj.status.disabled_subdued
                {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.y, pos.z))
            })
            .collect();
        if emitters.is_empty() {
            return;
        }

        let mut hits: Vec<(ObjectId, ObjectId, f32)> = Vec::new();
        for (eid, eteam, ex, _ey, ez) in &emitters {
            for (tid, tobj) in &self.objects {
                if tid == eid || !tobj.is_alive() {
                    continue;
                }
                let is_structure =
                    tobj.is_kind_of(KindOf::Structure) || tobj.object_type == ObjectType::Building;
                let airborne =
                    tobj.is_kind_of(KindOf::Aircraft) || tobj.object_type == ObjectType::Aircraft;
                let same_team = *eteam == tobj.team;
                let neutral = tobj.team == Team::Neutral;
                if !is_legal_microwave_emitter_target(
                    true,
                    airborne,
                    is_structure,
                    same_team,
                    neutral,
                ) {
                    continue;
                }
                let tpos = tobj.get_position();
                if !in_microwave_range_2d(
                    (*ex, *ez),
                    (tpos.x, tpos.z),
                    HOST_MICROWAVE_EMITTER_RADIUS,
                ) {
                    continue;
                }
                let dist = {
                    let dx = tpos.x - *ex;
                    let dz = tpos.z - *ez;
                    (dx * dx + dz * dz).sqrt()
                };
                let dmg = microwave_emitter_damage_at(dist);
                if dmg > 0.0 {
                    hits.push((*tid, *eid, dmg));
                }
            }
        }

        let mut applications = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let mut any_pos = None;
        for (tid, src, dmg) in hits {
            if let Some(o) = self.objects.get_mut(&tid) {
                if !o.is_alive() {
                    continue;
                }
                any_pos = Some(o.get_position());
                let killed = o.take_damage_from(dmg, Some(src));
                applications = applications.saturating_add(1);
                if killed {
                    destroy_ids.push((tid, Some(o.team)));
                }
            }
        }
        if applications > 0 {
            self.microwaves.record_emitter_damage(applications);
            let _ = HOST_MICROWAVE_EMITTER_FX;
            if let Some(pos) = any_pos {
                self.queue_audio_event(
                    AudioEventRequest::new(MICROWAVE_DISABLE_AUDIO)
                        .with_position(pos)
                        .with_priority(100),
                );
            }
        }
        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Host China Propaganda / Speaker Tower residual: heal + weapon buff in radius.
    ///
    /// C++ PropagandaTowerBehavior on ChinaSpeakerTower ModuleTag_06:
    /// Radius=150, HealPercentEachSecond=2% (4% upgraded), ENTHUSIASTIC / SUBLIMINAL.
    /// Fail-closed: continuous %max-health rate, same-team only (not full ally filter),
    /// non-structure only, no sole-benefactor exclusivity, no PulseFX.
    pub fn update_propaganda_tower_pulse(&mut self, dt: f32) {
        use crate::game_logic::host_propaganda::{
            in_propaganda_radius_2d, is_legal_propaganda_target, is_propaganda_tower,
            propaganda_heal_amount, HOST_PROPAGANDA_TOWER_RADIUS,
            UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
        };
        use std::collections::{HashMap, HashSet};

        if dt <= 0.0 {
            return;
        }

        // Snapshot towers: alive, fully built residual speaker/propaganda sources.
        // Includes Overlord/Helix propaganda addon flag + Emperor innate residual.
        use crate::game_logic::host_overlord_addons::{
            is_overlord_propaganda_source, overlord_propaganda_heal_amount,
            UPGRADE_OVERLORD_PROPAGANDA,
        };
        let towers: Vec<(ObjectId, Team, f32, f32, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let is_source = is_propaganda_tower(&obj.template_name)
                    || obj.has_overlord_propaganda_residual()
                    || is_overlord_propaganda_source(
                        obj.has_overlord_propaganda_addon,
                        &obj.template_name,
                    );
                if !obj.is_alive() || !is_source {
                    return None;
                }
                // C++: under construction towers do not pulse.
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                // Emperor UpgradeRequired residual uses OverlordPropagandaTower for upgraded rate.
                // Speaker towers use SubliminalMessaging for upgraded rate.
                let overlord_style = obj.has_overlord_propaganda_residual()
                    || crate::game_logic::host_overlord_addons::is_emperor_template(
                        &obj.template_name,
                    );
                let upgraded = if overlord_style {
                    obj.has_upgrade_tag(UPGRADE_OVERLORD_PROPAGANDA)
                        || obj.has_upgrade_tag("Upgrade_ChinaOverlordPropagandaTower")
                        || obj.has_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
                        || self
                            .players
                            .values()
                            .find(|p| p.team == obj.team)
                            .map(|p| {
                                p.unlocked_sciences.iter().any(|s| {
                                    s == UPGRADE_CHINA_SUBLIMINAL_MESSAGING
                                        || s == UPGRADE_OVERLORD_PROPAGANDA
                                })
                            })
                            .unwrap_or(false)
                } else {
                    obj.has_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
                        || self
                            .players
                            .values()
                            .find(|p| p.team == obj.team)
                            .map(|p| {
                                p.unlocked_sciences
                                    .iter()
                                    .any(|s| s == UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
                            })
                            .unwrap_or(false)
                };
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z, upgraded, overlord_style))
            })
            .collect();

        if towers.is_empty() {
            // Clear residual buffs when no towers remain.
            for obj in self.objects.values_mut() {
                if obj.weapon_bonus_enthusiastic || obj.weapon_bonus_subliminal {
                    obj.weapon_bonus_enthusiastic = false;
                    obj.record_host_weapon_bonus();
                    obj.weapon_bonus_subliminal = false;
                    obj.record_host_weapon_bonus();
                }
            }
            return;
        }

        // Snapshot non-structure candidates (heal if damaged; always eligible for buff).
        let candidates: Vec<(ObjectId, Team, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.is_kind_of(KindOf::Structure) {
                    return None;
                }
                if obj.status.under_construction {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z))
            })
            .collect();

        // Coverage map: target -> (upgraded, overlord_style heal rates).
        let mut coverage: HashMap<ObjectId, (bool, bool)> = HashMap::new();
        for (tower_id, tower_team, tx, tz, upgraded, overlord_style) in &towers {
            for (target_id, target_team, cx, cz) in &candidates {
                // Emperor AffectsSelf residual: allow self when overlord_style tower is self.
                let is_self = *tower_id == *target_id;
                if is_self && !*overlord_style {
                    continue;
                }
                if !is_legal_propaganda_target(
                    false,
                    true,
                    *tower_team == *target_team,
                    is_self && !*overlord_style,
                    false,
                ) {
                    continue;
                }
                if !in_propaganda_radius_2d((*tx, *tz), (*cx, *cz), HOST_PROPAGANDA_TOWER_RADIUS) {
                    continue;
                }
                let entry = coverage.entry(*target_id).or_insert((false, false));
                entry.0 = entry.0 || *upgraded;
                entry.1 = entry.1 || *overlord_style;
            }
        }

        let mut heal_ticks: u32 = 0;
        let mut buff_ticks: u32 = 0;
        let covered: HashSet<ObjectId> = coverage.keys().copied().collect();

        for (target_id, (upgraded, overlord_style)) in &coverage {
            let Some(target) = self.objects.get_mut(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }

            // ENTHUSIASTIC always while covered; SUBLIMINAL when upgraded cover present.
            let mut granted = false;
            if !target.weapon_bonus_enthusiastic {
                target.weapon_bonus_enthusiastic = true;
                target.record_host_weapon_bonus();
                granted = true;
            }
            if *upgraded {
                if !target.weapon_bonus_subliminal {
                    target.weapon_bonus_subliminal = true;
                    target.record_host_weapon_bonus();
                    granted = true;
                }
            } else if target.weapon_bonus_subliminal {
                target.weapon_bonus_subliminal = false;
                target.record_host_weapon_bonus();
            }
            if granted {
                buff_ticks = buff_ticks.saturating_add(1);
            }

            // %max-health heal residual (upgraded rate if any covering tower upgraded).
            // Overlord/Helix/Emperor use 1%/2%; speaker towers use 2%/4%.
            let max_hp = target.health.maximum.max(target.max_health);
            let heal_amt = if *overlord_style {
                overlord_propaganda_heal_amount(max_hp, *upgraded, dt)
            } else {
                propaganda_heal_amount(max_hp, *upgraded, dt)
            };
            if heal_amt > 0.0 {
                let before = target.health.current;
                if before + 0.01 < target.health.maximum {
                    target.heal(heal_amt);
                    if target.health.current > before + 0.0001 {
                        heal_ticks = heal_ticks.saturating_add(1);
                    }
                }
            }
        }

        // Clear buffs on units no longer covered by any tower.
        for (id, obj) in self.objects.iter_mut() {
            if covered.contains(id) {
                continue;
            }
            if obj.weapon_bonus_enthusiastic || obj.weapon_bonus_subliminal {
                obj.weapon_bonus_enthusiastic = false;
                obj.record_host_weapon_bonus();
                obj.weapon_bonus_subliminal = false;
                obj.record_host_weapon_bonus();
            }
        }

        for _ in 0..heal_ticks {
            self.record_propaganda_residual_heal();
        }
        for _ in 0..buff_ticks {
            self.record_propaganda_residual_buff();
        }
    }

    /// Host USA Ambulance AutoHeal residual: heal damaged ally infantry + vehicles in radius.
    ///
    /// C++ AutoHealBehavior on AmericaVehicleMedic:
    /// - ModuleTag_22: HealingAmount=4, HealingDelay=1000ms, Radius=100, KindOf=INFANTRY.
    /// - ModuleTag_23: HealingAmount=5, HealingDelay=1000ms, Radius=100, KindOf=VEHICLE,
    ///   ForbiddenKindOf=AIRCRAFT, SkipSelfForHealing=Yes.
    /// Fail-closed: continuous flat rate, same-team only (not full ally relationship filter),
    /// first-healer-wins sole-benefactor residual (not multi-module pulse phase).
    pub fn update_ambulance_auto_heal(&mut self, dt: f32) {
        use crate::game_logic::host_heal::{
            in_heal_radius_2d, is_ambulance_healer, is_legal_ambulance_infantry_heal_target,
            is_legal_ambulance_vehicle_heal_target, HostAmbulanceHealExclusivity,
            HOST_AMBULANCE_HEAL_RADIUS, HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC,
            HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC,
        };

        if dt <= 0.0 {
            return;
        }

        let infantry_heal = HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC * dt;
        let vehicle_heal = HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC * dt;
        if infantry_heal <= 0.0 && vehicle_heal <= 0.0 {
            return;
        }

        // Snapshot healers: alive ambulance/medic residual units.
        let healers: Vec<(ObjectId, Team, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_ambulance_healer(&obj.template_name) {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z))
            })
            .collect();

        if healers.is_empty() {
            return;
        }

        // Snapshot damaged candidates: infantry (ModuleTag_22) or ground vehicles (ModuleTag_23).
        // Kind residual: infantry OR (vehicle && !aircraft).
        let candidates: Vec<(ObjectId, Team, f32, f32, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let is_infantry = obj.is_kind_of(KindOf::Infantry);
                let is_vehicle =
                    obj.is_kind_of(KindOf::Vehicle) || obj.object_type == ObjectType::Vehicle;
                let is_aircraft = obj.is_kind_of(KindOf::Aircraft)
                    || obj.object_type == ObjectType::Aircraft
                    || obj.status.airborne_target;
                if !is_infantry && !(is_vehicle && !is_aircraft) {
                    return None;
                }
                let damaged = obj.health.current + 0.01 < obj.health.maximum;
                if !damaged {
                    return None;
                }
                let pos = obj.get_position();
                Some((
                    *id,
                    obj.team,
                    pos.x,
                    pos.z,
                    is_infantry,
                    is_vehicle,
                    is_aircraft,
                ))
            })
            .collect();

        if candidates.is_empty() {
            return;
        }

        // First-healer-wins residual (Wave 48 sole-benefactor map).
        let mut exclusivity = HostAmbulanceHealExclusivity::new();
        let mut heal_ticks: u32 = 0;
        for (healer_id, healer_team, hx, hz) in &healers {
            for (target_id, target_team, tx, tz, is_infantry, is_vehicle, is_aircraft) in
                &candidates
            {
                let same_team = *healer_team == *target_team;
                let is_self = *healer_id == *target_id;
                let legal = if *is_infantry {
                    is_legal_ambulance_infantry_heal_target(true, true, true, same_team, is_self)
                } else {
                    is_legal_ambulance_vehicle_heal_target(
                        *is_vehicle,
                        *is_aircraft,
                        true,
                        true,
                        same_team,
                        is_self,
                    )
                };
                if !legal {
                    continue;
                }
                if !in_heal_radius_2d((*hx, *hz), (*tx, *tz), HOST_AMBULANCE_HEAL_RADIUS) {
                    continue;
                }
                // Sole-benefactor residual: first ambulance wins this pulse.
                if !exclusivity.try_claim(*target_id, *healer_id) {
                    continue;
                }
                let amount = if *is_infantry {
                    infantry_heal
                } else {
                    vehicle_heal
                };
                if amount <= 0.0 {
                    continue;
                }
                if let Some(target) = self.objects.get_mut(target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let before = target.health.current;
                    if before + 0.01 >= target.health.maximum {
                        continue;
                    }
                    target.heal(amount);
                    if target.health.current > before + 0.0001 {
                        heal_ticks = heal_ticks.saturating_add(1);
                    }
                }
            }
        }

        for _ in 0..heal_ticks {
            self.record_ambulance_residual_heal();
        }
    }

    // -----------------------------------------------------------------------
    // Hero special-ability residual (snipe / timed C4 / cash hack)
    // Fail-closed: not full SpecialAbilityUpdate preparation / flee / upgrade matrix.
    // -----------------------------------------------------------------------

    /// Host hero special-ability residual registry (honesty counters).
    pub fn hero_abilities(
        &self,
    ) -> &crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry {
        &self.hero_abilities
    }

    /// Residual honesty: Jarmen Kell snipe unmanned a vehicle at least once.
    pub fn honesty_snipe_vehicle_ok(&self) -> bool {
        self.hero_abilities.honesty_snipe_ok()
    }

    /// Residual honesty: Burton planted a timed demo charge via special ability.
    pub fn honesty_plant_timed_demo_charge_ok(&self) -> bool {
        self.hero_abilities.honesty_timed_charge_plant_ok()
    }

    /// Residual honesty: Burton planted a remote demo charge via special ability.
    pub fn honesty_plant_remote_demo_charge_ok(&self) -> bool {
        self.hero_abilities.honesty_remote_charge_plant_ok()
    }

    /// Residual honesty: plant remote charge → detonate remote charges path.
    pub fn honesty_remote_demo_charge_detonate_ok(&self) -> bool {
        self.hero_abilities.honesty_remote_charge_detonate_ok()
    }

    /// Residual honesty: Black Lotus cash-hack completed at least once.
    pub fn honesty_steal_cash_ok(&self) -> bool {
        self.hero_abilities.honesty_cash_steal_ok()
    }

    /// Residual honesty: Black Lotus / hero CaptureBuilding completed at least once.
    pub fn honesty_black_lotus_capture_ok(&self) -> bool {
        self.hero_abilities.honesty_building_capture_ok()
    }

    /// Host black market residual registry (deposits + honesty).
    pub fn black_markets(&self) -> &crate::game_logic::host_black_market::HostBlackMarketRegistry {
        &self.black_markets
    }

    /// Residual honesty: GLA Black Market AutoDeposit residual deposited cash.
    pub fn honesty_black_market_deposit_ok(&self) -> bool {
        self.black_markets.honesty_deposit_ok()
    }

    /// Host oil derrick residual registry (deposits + capture bonus + honesty).
    pub fn oil_derricks(&self) -> &crate::game_logic::host_oil_derrick::HostOilDerrickRegistry {
        &self.oil_derricks
    }

    /// Host hacker income residual registry (deposits + honesty).
    pub fn hacker_income(
        &self,
    ) -> &crate::game_logic::host_hacker_income::HostHackerIncomeRegistry {
        &self.hacker_income
    }

    /// Host Supply Drop Zone residual registry (drops + honesty).
    pub fn supply_drop_zones(
        &self,
    ) -> &crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry {
        &self.supply_drop_zones
    }

    /// Host CommandCenter / RadarVan radar residual registry.
    pub fn host_radar(&self) -> &crate::game_logic::host_radar::HostRadarRegistry {
        &self.host_radar
    }

    /// Residual honesty: player radar came online via CC / RadarVan ownership.
    pub fn honesty_radar_provider_online_ok(&self) -> bool {
        self.host_radar.honesty_online_ok()
    }

    /// Residual honesty: Black Lotus disable-vehicle hack completed at least once.
    pub fn honesty_disable_vehicle_hack_ok(&self) -> bool {
        self.hero_abilities.honesty_vehicle_disable_ok()
    }

    /// Combined residual honesty: any hero special ability path observed.
    pub fn honesty_hero_ability_ok(&self) -> bool {
        self.hero_abilities.honesty_any_ok()
    }
}
