//! Host combat `impl GameLogic` — `gps_and_fields`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use gamelogic::common::Relationship;
impl GameLogic {
    /// Activate GPS Scrambler residual: GrantStealth to ally vehicles/infantry in radius.
    ///
    /// Matches retail SuperweaponGPSScrambler → GPSScrambler_InvisibleMarker:
    /// - FinalRadius residual 100 (RadiusCursorRadius / GrantStealth FinalRadius)
    /// - KindOf VEHICLE | INFANTRY, C++ ALLOW_ALLIES (same player or allied players)
    /// - receiveGrant when the target has StealthUpdate (C++ getStealth()):
    ///   authored innate_stealth **or** default VEHICLE|INFANTRY module
    ///   (ThingTemplate.cpp:384-409), then Drawable::flashAsSelected
    /// - Skips bomb-truck disguise residual by name (C++ canDisguise skip)
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_gps_scrambler(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_gps_scrambler::{
            GPS_SCRAMBLER_ACTIVATE_AUDIO, GPS_SCRAMBLER_INVISIBLE_MARKER,
            GPS_SCRAMBLER_START_RADIUS, HOST_GPS_SCRAMBLER_RADIUS, HostGpsScrambler,
            in_gps_scrambler_radius_2d, is_gps_scrambler_disguise_name,
            is_legal_gps_scrambler_target,
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

        let candidates: Vec<(ObjectId, bool, bool, bool, bool, bool, bool)> = self
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
                let is_ally = self.gps_grant_is_ally(player_id, caster_id, caster_team, obj);
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let is_disguise = is_gps_scrambler_disguise_name(&obj.template_name);
                let has_stealth_module = obj.has_gps_stealth_module();
                Some((
                    *id,
                    is_vehicle,
                    is_infantry,
                    is_ally,
                    under_construction,
                    is_disguise,
                    has_stealth_module,
                ))
            })
            .collect();

        let mut grants: u32 = 0;
        for (
            id,
            is_vehicle,
            is_infantry,
            is_ally,
            under_construction,
            is_disguise,
            has_stealth_module,
        ) in candidates
        {
            if !is_legal_gps_scrambler_target(
                is_vehicle,
                is_infantry,
                true,
                is_ally,
                under_construction,
                is_disguise,
                has_stealth_module,
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
            // C++ grantStealthToObject: receiveGrant() then draw->flashAsSelected().
            target.flash_as_selected();
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
        self.try_eva_special_launched_misc_owned(Some(player_id), caster_team, "gps");

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

    /// C++ PartitionFilterRelationship(self, ALLOW_ALLIES).
    /// Same player or allied players; leftover fallback is same non-neutral team.
    pub(in super::super) fn gps_grant_is_ally(
        &self,
        caster_player_id: u32,
        caster_id: Option<ObjectId>,
        caster_team: Team,
        obj: &Object,
    ) -> bool {
        use gamelogic::common::Relationship;
        let caster_owner = caster_id
            .and_then(|id| self.objects.get(&id))
            .and_then(|c| self.player_owner_for_host_object(c))
            .or(Some(caster_player_id));
        let obj_owner = self.player_owner_for_host_object(obj);
        match (caster_owner, obj_owner) {
            (Some(a), Some(b)) => self.player_relationship(a, b) == Relationship::Allies,
            _ => obj.team == caster_team && caster_team != Team::Neutral,
        }
    }

    /// Host China ECM Tank / jammer residual: jam enemy weapons in radius.
    ///
    /// C++ `ECMTankVehicleDisabler` (SUBDUAL_VEHICLE, AttackRange 200, 24/100ms):
    /// ActiveBody.cpp:471-487 accumulates (no HP); `onSubdualChange` sets
    /// `DISABLED_SUBDUED` when `isSubdued()` (`maxHealth <= subdual`, :1292-1294).
    /// AIUpdate only processes `DISABLED_HELD`, so movement/AI halt. Live
    /// `weapons_jammed` stays fire-only (`canFireWeapon` / JAMMED mesh).
    /// Infantry/aircraft are not targets. SubdualDamageHelper.cpp:32-50 heals
    /// so disable lingers after leaving.
    pub fn update_ecm_jam_field(&mut self) {
        use crate::game_logic::host_ecm_jam::{
            ECM_VEHICLE_DISABLER_ATTACK_RANGE, ECM_VEHICLE_DISABLER_DELAY_FRAMES,
            ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE, in_ecm_jam_radius_2d, is_ecm_hostile_team,
            is_ecm_jammer, is_legal_ecm_vehicle_disabler_target, seed_host_subdual_if_unauthored,
        };
        use std::collections::HashSet;

        // Snapshot jammers: alive ECM tank / frequency jammer residual sources.
        let jammers: Vec<(ObjectId, Team, Option<u32>, String, f32, f32)> = self
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
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.team_instance_name.clone(),
                    pos.x,
                    pos.z,
                ))
            })
            .collect();

        if jammers.is_empty() {
            // Linger: C++ SubdualDamageHelper heals then onSubdualChange clears.
            for obj in self.objects.values_mut() {
                let vehicle = obj.is_kind_of(KindOf::Vehicle)
                    && !obj.is_kind_of(KindOf::Aircraft)
                    && obj.object_type != ObjectType::Aircraft;
                let full = obj.is_subdued();
                // weapons_jammed is fire-only; clear when the bar is no longer full.
                if obj.status.weapons_jammed && !full {
                    obj.set_weapons_jammed(false);
                }
                // DISABLED_SUBDUED is a full halt. Only vehicles here — microwave
                // structures use the same flag via update_microwave_disable_field.
                if vehicle && obj.status.disabled_subdued && !full {
                    obj.set_disabled_subdued(false);
                }
            }
            return;
        }

        // C++ ECMTankVehicleDisabler: ground vehicles only (not infantry/aircraft).
        let candidates: Vec<(ObjectId, Option<u32>, String, f32, f32, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                if obj.status.under_construction {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_aircraft =
                    obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft;
                if !is_vehicle || is_aircraft {
                    return None;
                }
                let pos = obj.get_position();
                Some((
                    *id,
                    obj.owner_player_id,
                    obj.team_instance_name.clone(),
                    pos.x,
                    pos.z,
                    is_vehicle,
                    is_aircraft,
                ))
            })
            .collect();

        let mut covered: HashSet<ObjectId> = HashSet::new();
        // Jammer → target links for ECMDisableStream laser residual.
        let mut jam_links: Vec<(ObjectId, ObjectId)> = Vec::new();
        for (jammer_id, jammer_team, jammer_owner, jammer_team_instance, jx, jz) in &jammers {
            let jammer_neutral = *jammer_team == Team::Neutral;
            for (target_id, target_owner, target_team_instance, tx, tz, is_vehicle, is_aircraft) in
                &candidates
            {
                let rel = GameLogic::object_relationship_from_owners(
                    &self.players,
                    *target_owner,
                    target_team_instance,
                    *jammer_owner,
                    jammer_team_instance,
                );
                let same_team = rel == Relationship::Allies;
                let target_neutral = rel == Relationship::Neutral;
                let enemy_or_neutral =
                    is_ecm_hostile_team(jammer_neutral, same_team, target_neutral);
                if !is_legal_ecm_vehicle_disabler_target(
                    *is_vehicle,
                    *is_aircraft,
                    true,
                    enemy_or_neutral,
                    *jammer_id == *target_id,
                    false,
                ) {
                    continue;
                }
                if !in_ecm_jam_radius_2d((*jx, *jz), (*tx, *tz), ECM_VEHICLE_DISABLER_ATTACK_RANGE)
                {
                    continue;
                }
                covered.insert(*target_id);
                jam_links.push((*jammer_id, *target_id));
            }
        }

        let mut jam_ticks: u32 = 0;
        let pulse = self.frame % ECM_VEHICLE_DISABLER_DELAY_FRAMES.max(1) == 0;
        if pulse {
            // C++ WeaponSet fire updates FiringTracker lastShotFiredFrame.
            // ExclusiveWeaponDelay 1000ms → 30f then suppresses ECMTankMissileJammer.
            let mut fired: HashSet<ObjectId> = HashSet::new();
            for (jammer_id, _) in &jam_links {
                fired.insert(*jammer_id);
            }
            for jid in fired {
                if let Some(j) = self.objects.get_mut(&jid) {
                    j.last_fire_frame = self.frame;
                }
            }
            for target_id in &covered {
                let Some(target) = self.objects.get_mut(target_id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let max_h = target.health.maximum.max(target.max_health);
                seed_host_subdual_if_unauthored(
                    &mut target.subdual_damage_cap,
                    &mut target.subdual_heal_rate_frames,
                    &mut target.subdual_heal_amount,
                    max_h,
                );
                // C++ ActiveBody::internalAddSubdualDamage + onSubdualChange
                // → setDisabled(DISABLED_SUBDUED). AIUpdate only processes
                // DISABLED_HELD, so movement/AI halt. weapons_jammed stays fire-only.
                target.apply_subdual_damage(ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE);
            }
        }

        // Sync fire-only weapons_jammed + DISABLED_SUBDUED full halt from isSubdued.
        for obj in self.objects.values_mut() {
            let vehicle = obj.is_kind_of(KindOf::Vehicle)
                && !obj.is_kind_of(KindOf::Aircraft)
                && obj.object_type != ObjectType::Aircraft;
            let should = vehicle && obj.is_subdued();
            if should && !obj.status.disabled_subdued {
                obj.set_disabled_subdued(true);
            } else if !should && vehicle && obj.status.disabled_subdued {
                obj.set_disabled_subdued(false);
            }
            if should && !obj.status.weapons_jammed {
                obj.set_weapons_jammed(true);
                jam_ticks = jam_ticks.saturating_add(1);
            } else if !should && obj.status.weapons_jammed {
                obj.set_weapons_jammed(false);
            }
        }

        for _ in 0..jam_ticks {
            self.record_ecm_residual_jam();
        }

        // C++ LaserName ECMDisableStream residual (VehicleDisabler WEAPONA01 bone).
        // Cadence residual: DelayBetweenShots 100ms → 3f. ExclusiveWeaponDelay
        // stamps last_fire_frame so FireWeaponUpdate jammer stays suppressed.
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

    /// C++ ECMTankMissileJammer: SUBDUAL_MISSILE never subtracts HP
    /// (ActiveBody.cpp:471-487). `onSubdualChange` (:1280-1286) calls
    /// `MissileAIUpdate::projectileNowJammed` (MissileAIUpdate.cpp:777-809):
    /// MODELCONDITION_JAMMED, scatter `DistanceScatterWhenJammed` (default 75),
    /// clear tracking. `DumbProjectileBehavior::projectileNowJammed` is empty.
    pub fn update_ecm_missile_jam(&mut self) {
        use crate::game_logic::host_ecm_jam::{
            ECM_MISSILE_JAM_MAX_PER_PULSE, HOST_ECM_JAM_RADIUS, ecm_exclusive_weapon_delay_blocks,
            ecm_missile_scatter_offset, in_ecm_jam_radius_2d, is_dumb_projectile_shell_name,
            is_ecm_jam_projectile_flags, is_ecm_jammer,
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
                if ecm_exclusive_weapon_delay_blocks(frame, obj.last_fire_frame) {
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
                    || obj.humvee_tow_projectile;
                // C++ DumbProjectileBehavior::projectileNowJammed is empty.
                let dumb_shell = is_dumb_projectile_shell_name(&obj.template_name)
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
                if !in_flight || dumb_shell {
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

        for (mid, jammer_id, mx, mz) in jammed_ids {
            let seed = mid.0.wrapping_add(jammer_id.0).wrapping_add(frame);
            let (sx, sz) = ecm_missile_scatter_offset(seed);
            let new_aim = [mx + sx, 0.0, mz + sz];
            if let Some(o) = self.objects.get_mut(&mid) {
                if o.ecm_missile_jammed {
                    continue;
                }
                o.ecm_missile_jammed = true;
                // C++ projectileNowJammed: scatter + lose lock. No Unresistable HP.
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
            }
            self.ecm_missiles_jammed = self.ecm_missiles_jammed.saturating_add(1);
        }
    }

    pub fn honesty_ecm_missile_jam_ok(&self) -> bool {
        self.ecm_missiles_jammed > 0
    }

    pub fn update_microwave_disable(&mut self) {
        use crate::game_logic::host_microwave::{
            HOST_MICROWAVE_DELAY_FRAMES, HOST_MICROWAVE_DISABLE_RANGE,
            HOST_MICROWAVE_SUBDUAL_PULSE, MICROWAVE_DISABLE_AUDIO, in_microwave_range_2d,
            is_legal_microwave_disable_target, is_microwave_hostile_team, is_microwave_tank,
            seed_microwave_subdual_if_unauthored, should_microwave_disable,
        };
        use std::collections::HashSet;

        // Snapshot microwave tanks that are actively attacking.
        let cookers: Vec<(ObjectId, Team, Option<u32>, String, ObjectId, f32, f32)> = self
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
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.team_instance_name.clone(),
                    target_id,
                    pos.x,
                    pos.z,
                ))
            })
            .collect();

        let mut covered: HashSet<ObjectId> = HashSet::new();
        let mut first_grant_pos: Option<glam::Vec3> = None;

        for (_cooker_id, cooker_team, cooker_owner, cooker_team_instance, target_id, cx, cz) in
            &cookers
        {
            let Some(target) = self.objects.get(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let is_structure =
                target.is_kind_of(KindOf::Structure) || target.object_type == ObjectType::Building;
            let rel = GameLogic::object_relationship_from_owners(
                &self.players,
                target.owner_player_id,
                &target.team_instance_name,
                *cooker_owner,
                cooker_team_instance,
            );
            let same_team = rel == Relationship::Allies;
            let target_neutral = rel == Relationship::Neutral;
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
        let pulse = self.frame % HOST_MICROWAVE_DELAY_FRAMES.max(1) == 0;
        if pulse {
            for target_id in &covered {
                let Some(target) = self.objects.get_mut(target_id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let max_h = target.health.maximum.max(target.max_health);
                seed_microwave_subdual_if_unauthored(
                    &mut target.subdual_damage_cap,
                    &mut target.subdual_heal_rate_frames,
                    &mut target.subdual_heal_amount,
                    max_h,
                );
                let was = target.is_subdued();
                // C++ ActiveBody.cpp:471-487 SUBDUAL_BUILDING — no HP; disable
                // when currentSubdual >= maxHealth (:1292-1294).
                let _ = target.take_damage_from_typed(
                    HOST_MICROWAVE_SUBDUAL_PULSE,
                    None,
                    crate::game_logic::combat::DamageType::SubdualBuilding,
                );
                if !was && target.is_subdued() {
                    new_grants = new_grants.saturating_add(1);
                } else if target.is_subdued() {
                    refresh_ticks = refresh_ticks.saturating_add(1);
                }
                self.microwaves.record_disable_weapon_pulse();
            }
        }

        // Do not instantly clear when the beam drops — SubdualDamageHelper heals.
        let still_disabled = self
            .objects
            .values()
            .filter(|o| o.status.disabled_subdued && o.is_alive())
            .count() as u32;

        for _ in 0..new_grants {
            self.microwaves.record_disable_grant();
        }
        for _ in 0..refresh_ticks {
            self.microwaves.record_disable_refresh();
        }
        self.microwaves.set_currently_disabled(still_disabled);

        // C++ LaserName MicrowaveDisableStream residual attach (bone WEAPON02).
        // Spawn short-lived beam Things + presentation ResidualWeaponLaser per cook link.
        {
            use crate::game_logic::host_microwave::{
                HOST_MICROWAVE_LASER_BONE, HOST_MICROWAVE_LASER_NAME,
            };
            use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;

            let mut laser_links: Vec<(ObjectId, ObjectId, glam::Vec3, glam::Vec3)> = Vec::new();
            for (cooker_id, _team, _owner, _team_instance, target_id, _cx, _cz) in &cookers {
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

    /// C++ MicrowaveTankEmitterWeapon: DamageType MICROWAVE (Damage.h:63),
    /// PrimaryDamage 8, radius 100. TankArmor MICROWAVE is 0%; HumanArmor 100%.
    /// Not Unresistable — Crusaders/Battlemasters in the field take 0 HP.
    ///
    pub fn update_microwave_emitter_field(&mut self) {
        use crate::game_logic::host_microwave::{
            HOST_MICROWAVE_EMITTER_DELAY_FRAMES, HOST_MICROWAVE_EMITTER_FX,
            HOST_MICROWAVE_EMITTER_RADIUS, MICROWAVE_DISABLE_AUDIO, in_microwave_range_2d,
            is_legal_microwave_emitter_target, is_microwave_tank, microwave_emitter_damage_at,
        };

        let frame = self.frame;
        // Pulse cadence residual (DelayBetweenShots 250ms → 8f).
        if frame % HOST_MICROWAVE_EMITTER_DELAY_FRAMES.max(1) != 0 {
            return;
        }

        let emitters: Vec<(ObjectId, Option<u32>, String, f32, f32, f32)> = self
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
                Some((
                    *id,
                    obj.owner_player_id,
                    obj.team_instance_name.clone(),
                    pos.x,
                    pos.y,
                    pos.z,
                ))
            })
            .collect();
        if emitters.is_empty() {
            return;
        }

        let mut hits: Vec<(ObjectId, ObjectId, f32)> = Vec::new();
        for (eid, eowner, eteam_instance, ex, _ey, ez) in &emitters {
            for (tid, tobj) in &self.objects {
                if tid == eid || !tobj.is_alive() {
                    continue;
                }
                let is_structure =
                    tobj.is_kind_of(KindOf::Structure) || tobj.object_type == ObjectType::Building;
                let airborne =
                    tobj.is_kind_of(KindOf::Aircraft) || tobj.object_type == ObjectType::Aircraft;
                let rel = GameLogic::object_relationship_from_owners(
                    &self.players,
                    tobj.owner_player_id,
                    &tobj.team_instance_name,
                    *eowner,
                    eteam_instance,
                );
                let same_team = rel == Relationship::Allies;
                let neutral = rel == Relationship::Neutral;
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
                let killed = o.take_damage_from_immediate_typed(
                    dmg,
                    Some(src),
                    crate::game_logic::combat::DamageType::Microwave,
                );
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
    /// Membership follows `m_scanDelayInFrames` (2000ms). Sold / double-contained
    /// sources `removeAllInfluence`. ALLOW_ALLIES. SCORE + hasAnyDamageWeapon.
    /// Enclosed riders (`contained_by`) are not partition-registered
    /// (C++ OpenContain::addToContain → unRegisterObject); live snaps them
    /// to the hull so they must be excluded from the radius scan.
    pub fn update_propaganda_tower_pulse(&mut self, dt: f32) {
        use crate::game_logic::host_overlord_addons::{
            is_overlord_propaganda_source, overlord_propaganda_heal_amount,
        };
        use crate::game_logic::host_propaganda::{
            HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES, HOST_PROPAGANDA_TOWER_RADIUS,
            PROPAGANDA_PULSE_FX, PROPAGANDA_UPGRADED_PULSE_FX, UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
            host_has_any_damage_weapon, in_propaganda_radius_2d, is_legal_propaganda_target,
            is_portable_propaganda_structure, is_propaganda_score_kind, is_propaganda_tower,
            is_subliminal_upgrade_active, propaganda_applies_weapon_bonus, propaganda_heal_amount,
            propaganda_source_suppressed, should_play_propaganda_pulse_fx,
        };
        use gamelogic::common::Relationship;
        use std::collections::{HashMap, HashSet};

        if dt <= 0.0 {
            return;
        }

        struct TowerSnap {
            id: ObjectId,
            team: Team,
            x: f32,
            z: f32,
            upgraded: bool,
            overlord_style: bool,
            owner: Option<u32>,
            stealthed: bool,
            detected: bool,
            contained_by: Option<ObjectId>,
        }

        let mut inactive: Vec<ObjectId> = Vec::new();
        let mut towers: Vec<TowerSnap> = Vec::new();
        for (id, obj) in &self.objects {
            let is_source = is_propaganda_tower(&obj.template_name)
                || obj.has_overlord_propaganda_residual()
                || is_overlord_propaganda_source(
                    obj.has_overlord_propaganda_addon,
                    &obj.template_name,
                );
            if !is_source {
                continue;
            }
            let container_nested = obj
                .contained_by
                .and_then(|cid| self.objects.get(&cid))
                .and_then(|c| c.contained_by)
                .is_some();
            let suppressed = propaganda_source_suppressed(
                obj.contained_by.is_some(),
                container_nested,
                obj.is_kind_of(KindOf::Vehicle),
                is_portable_propaganda_structure(&obj.template_name),
            );
            if !obj.is_alive()
                || obj.status.under_construction
                || obj.construction_percent + 0.001 < 1.0
                || obj.status.sold
                || obj.is_disabled()
                || suppressed
            {
                inactive.push(*id);
                continue;
            }
            let overlord_style = obj.has_overlord_propaganda_residual()
                || crate::game_logic::host_overlord_addons::is_emperor_template(&obj.template_name);
            let controlling = obj.owner_player_id.and_then(|pid| {
                self.players
                    .get(&pid)
                    .filter(|p| p.is_alive && p.team == obj.team)
            });
            let player_has = |names: &[&str]| {
                controlling.is_some_and(|p| {
                    names.iter().any(|name| {
                        p.unlocked_sciences.iter().any(|s| s == *name)
                            || p.completed_upgrades.iter().any(|s| s == *name)
                    })
                })
            };
            // C++ PropagandaTowerBehavior::effectLogic:275
            // getControllingPlayer()->hasUpgradeComplete(m_upgradeRequired)
            // with UpgradeRequired=Upgrade_ChinaSubliminalMessaging. Addon
            // install tags (Overlord/Helix propaganda) are not this upgrade.
            let upgraded =
                is_subliminal_upgrade_active(player_has(&[UPGRADE_CHINA_SUBLIMINAL_MESSAGING]));
            let pos = obj.get_position();
            towers.push(TowerSnap {
                id: *id,
                team: obj.team,
                x: pos.x,
                z: pos.z,
                upgraded,
                overlord_style,
                owner: obj.owner_player_id,
                stealthed: obj.status.stealthed,
                detected: obj.status.detected,
                contained_by: obj.contained_by,
            });
        }

        for id in inactive {
            let _ = self.propaganda_scan.take_tower(id);
        }

        if towers.is_empty() {
            self.propaganda_scan.clear();
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

        let candidates: Vec<(ObjectId, Team, f32, f32, Option<u32>)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.is_kind_of(KindOf::Structure) {
                    return None;
                }
                if obj.status.under_construction {
                    return None;
                }
                // C++ OpenContain.cpp:320-322: enclosing contain unregisters
                // the rider from PartitionManager, so doScan never sees them.
                if obj.contained_by.is_some() {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, pos.x, pos.z, obj.owner_player_id))
            })
            .collect();

        let frame = self.frame;
        let delay = HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES;
        let local_pid = self.local_player_id();
        let due: Vec<ObjectId> = towers
            .iter()
            .filter(|t| self.propaganda_scan.should_scan(t.id, frame, delay))
            .map(|t| t.id)
            .collect();

        for tower in &towers {
            if !due.iter().any(|id| *id == tower.id) {
                continue;
            }
            let mut new_inside = Vec::new();
            for (target_id, target_team, cx, cz, target_owner) in &candidates {
                let is_self = *target_id == tower.id;
                if is_self && !tower.overlord_style {
                    continue;
                }
                let is_ally = match (tower.owner, *target_owner) {
                    (Some(a), Some(b)) => self.player_relationship(a, b) == Relationship::Allies,
                    _ => tower.team == *target_team && tower.team != Team::Neutral,
                };
                if !is_legal_propaganda_target(
                    false,
                    true,
                    is_ally,
                    is_self && !tower.overlord_style,
                    false,
                ) {
                    continue;
                }
                if !in_propaganda_radius_2d(
                    (tower.x, tower.z),
                    (*cx, *cz),
                    HOST_PROPAGANDA_TOWER_RADIUS,
                ) {
                    continue;
                }
                new_inside.push(*target_id);
            }
            self.propaganda_scan.set_inside(tower.id, new_inside);
            self.propaganda_scan.mark_scanned(tower.id, frame);

            let container = tower.contained_by.and_then(|cid| self.objects.get(&cid));
            let do_fx = should_play_propaganda_pulse_fx(
                local_pid.is_some() && local_pid == tower.owner,
                tower.stealthed,
                tower.detected,
                tower.contained_by.is_some(),
                container.is_some_and(|c| c.status.stealthed),
                container.is_some_and(|c| c.status.detected),
            );
            if do_fx {
                let fx = if tower.upgraded {
                    PROPAGANDA_UPGRADED_PULSE_FX
                } else {
                    PROPAGANDA_PULSE_FX
                };
                // C++ PropagandaTowerBehavior.cpp:450-456 doFXObj(pulse, us).
                let _ = self.dispatch_fx_list_at_host_object(fx, tower.id, None);
            }
        }

        let mut coverage: HashMap<ObjectId, (bool, bool)> = HashMap::new();
        for tower in &towers {
            for &tid in self.propaganda_scan.inside(tower.id) {
                let entry = coverage.entry(tid).or_insert((false, false));
                entry.0 |= tower.upgraded;
                entry.1 |= tower.overlord_style;
            }
        }
        let covered: HashSet<ObjectId> = coverage.keys().copied().collect();

        let mut heal_ticks: u32 = 0;
        let mut buff_ticks: u32 = 0;
        for (target_id, (upgraded, overlord_style)) in &coverage {
            let Some(target) = self.objects.get_mut(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            if !is_propaganda_score_kind(
                target.is_kind_of(KindOf::Score),
                target.is_kind_of(KindOf::ScoreCreate),
                target.is_kind_of(KindOf::ScoreDestroy),
                target.is_kind_of(KindOf::MpCountForVictory),
            ) {
                continue;
            }

            let unarmed_worker = target.is_kind_of(KindOf::Dozer)
                || target.is_kind_of(KindOf::Worker)
                || target.is_kind_of(KindOf::Harvester);
            let authored = target.thing.template.primary_weapon_name.is_some()
                || target.thing.template.secondary_weapon_name.is_some()
                || target.thing.template.tertiary_weapon_name.is_some();
            let bound_damage = [0u8, 1, 2]
                .into_iter()
                .any(|slot| target.weapon_slot(slot).is_some_and(|w| w.damage > 0.0));

            if propaganda_applies_weapon_bonus(host_has_any_damage_weapon(
                bound_damage,
                unarmed_worker,
                authored,
            )) {
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
            }

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
    /// Pulse every HealingDelay (not continuous HP/s). ALLOW_ALLIES (player relationship,
    /// leftover same-team fallback). Sole-benefactor lasts HealingDelay frames.
    pub fn update_ambulance_auto_heal(&mut self, dt: f32) {
        use crate::game_logic::host_heal::{
            AMBULANCE_HEAL_DELAY_FRAMES, AMBULANCE_INFANTRY_HEAL_AMOUNT,
            AMBULANCE_VEHICLE_HEAL_AMOUNT, HOST_AMBULANCE_HEAL_RADIUS, ambulance_heal_is_ally,
            ambulance_pulse_ready, in_heal_radius_2d, is_ambulance_healer,
            is_legal_ambulance_infantry_heal_target, is_legal_ambulance_vehicle_heal_target,
        };
        use gamelogic::common::Relationship;

        if !ambulance_pulse_ready(&mut self.ambulance_auto_heal_pulse_accum, dt) {
            return;
        }

        // Snapshot healers: alive ambulance/medic residual units.
        let healers: Vec<(ObjectId, Team, Option<u32>, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_ambulance_healer(&obj.template_name) {
                    return None;
                }
                let pos = obj.get_position();
                Some((*id, obj.team, obj.owner_player_id, pos.x, pos.z))
            })
            .collect();

        if healers.is_empty() {
            return;
        }

        // Snapshot damaged candidates: infantry (ModuleTag_22) or ground vehicles (ModuleTag_23).
        let candidates: Vec<(ObjectId, Team, Option<u32>, f32, f32, bool, bool, bool)> = self
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
                    obj.owner_player_id,
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

        let now = self.frame as u32;
        let mut heal_ticks: u32 = 0;
        for (healer_id, healer_team, healer_owner, hx, hz) in &healers {
            for (
                target_id,
                target_team,
                target_owner,
                tx,
                tz,
                is_infantry,
                is_vehicle,
                is_aircraft,
            ) in &candidates
            {
                let same_team = *healer_team == *target_team;
                let player_allies = match (healer_owner, target_owner) {
                    (Some(a), Some(b)) => {
                        Some(self.player_relationship(*a, *b) == Relationship::Allies)
                    }
                    _ => None,
                };
                let allies = ambulance_heal_is_ally(same_team, player_allies);
                let is_self = *healer_id == *target_id;
                let legal = if *is_infantry {
                    is_legal_ambulance_infantry_heal_target(true, true, true, allies, is_self)
                } else {
                    is_legal_ambulance_vehicle_heal_target(
                        *is_vehicle,
                        *is_aircraft,
                        true,
                        true,
                        allies,
                        is_self,
                    )
                };
                if !legal {
                    continue;
                }
                if !in_heal_radius_2d((*hx, *hz), (*tx, *tz), HOST_AMBULANCE_HEAL_RADIUS) {
                    continue;
                }
                let amount = if *is_infantry {
                    AMBULANCE_INFANTRY_HEAL_AMOUNT
                } else {
                    AMBULANCE_VEHICLE_HEAL_AMOUNT
                };
                if amount <= 0.0 {
                    continue;
                }
                if let Some(target) = self.objects.get_mut(target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    if target.attempt_healing_from_sole_benefactor(
                        amount,
                        *healer_id,
                        AMBULANCE_HEAL_DELAY_FRAMES,
                        now,
                    ) {
                        heal_ticks = heal_ticks.saturating_add(1);
                    }
                }
            }
        }

        for _ in 0..heal_ticks {
            self.record_ambulance_residual_heal();
        }
    }

    /// Host leftover DefaultAutoHealBehavior: trainable units self-heal after StartHealingDelay.
    /// HELD / garrisoned units still pulse (C++ DISABLED_HELD).
    pub fn update_default_auto_heal(&mut self) {
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.default_auto_heal.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
            let cur = obj.health.current;
            let amount = {
                let Some(ah) = obj.default_auto_heal.as_mut() else {
                    continue;
                };
                ah.tick_heal_amount(frame, cur, max_h)
            };
            if amount > 0.0 {
                obj.heal(amount);
            }
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

    /// Restore leftover SpecialAbilityUpdate channels after load.
    pub fn hero_abilities_mut(
        &mut self,
    ) -> &mut crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry {
        &mut self.hero_abilities
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, ThingTemplate};

    fn insert_speaker_and_infantry(logic: &mut GameLogic) {
        let mut tower_tpl = ThingTemplate::new("ChinaSpeakerTower");
        tower_tpl
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0)
            .set_cost(500, 0);
        logic
            .templates
            .insert("ChinaSpeakerTower".to_string(), tower_tpl);

        let mut inf = ThingTemplate::new("TestInfantry");
        inf.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Score)
            .set_health(80.0)
            .set_cost(0, 0);
        logic.templates.insert("TestInfantry".to_string(), inf);
    }

    /// C++ PropagandaTowerBehavior.cpp:177-188 — EMP/underpowered (not HELD)
    /// stops the pulse and removeAllInfluence clears ENTHUSIASTIC.
    #[test]
    fn propaganda_pulse_skips_disabled_except_held_and_removes_influence() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);

        let tower_id = logic
            .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let unit_id = logic
            .create_object("TestInfantry", Team::China, Vec3::new(20.0, 0.0, 0.0))
            .expect("unit");
        {
            let unit = logic.host_object_mut(unit_id).expect("unit");
            let _ = unit.take_damage(40.0);
        }

        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                unit.weapon_bonus_enthusiastic,
                "enabled tower must grant ENTHUSIASTIC"
            );
        }
        let hp_after_pulse = logic.host_object(unit_id).expect("unit").health.current;

        {
            let tower = logic.host_object_mut(tower_id).expect("tower");
            tower.set_status_disabled_emp(true);
            assert!(tower.is_disabled());
        }
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                !unit.weapon_bonus_enthusiastic,
                "disabled tower must removeAllInfluence (clear ENTHUSIASTIC)"
            );
            assert!(!unit.weapon_bonus_subliminal);
            assert!(
                (unit.health.current - hp_after_pulse).abs() < 0.01,
                "disabled tower must not keep healing"
            );
        }

        // HELD-only (contained) is not is_disabled — pulse continues.
        {
            let tower = logic.host_object_mut(tower_id).expect("tower");
            tower.set_status_disabled_emp(false);
            tower.contained_by = Some(ObjectId(99));
            assert!(!tower.is_disabled());
        }
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                unit.weapon_bonus_enthusiastic,
                "HELD-only tower must keep pulsing (C++ DISABLED_HELD exception)"
            );
        }
    }

    /// C++ PropagandaTowerBehavior.cpp:168-171 — OBJECT_STATUS_SOLD strips
    /// influence immediately, even while the sell animation is still playing.
    #[test]
    fn propaganda_pulse_skips_sold_tower_and_removes_influence() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);

        let tower_id = logic
            .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let unit_id = logic
            .create_object("TestInfantry", Team::China, Vec3::new(20.0, 0.0, 0.0))
            .expect("unit");
        {
            let unit = logic.host_object_mut(unit_id).expect("unit");
            let _ = unit.take_damage(40.0);
        }

        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                unit.weapon_bonus_enthusiastic,
                "enabled tower must grant ENTHUSIASTIC"
            );
        }
        let hp_after_pulse = logic.host_object(unit_id).expect("unit").health.current;

        {
            let tower = logic.host_object_mut(tower_id).expect("tower");
            tower.set_status_sold(true);
            assert!(tower.is_alive(), "sold tower is still alive during sell");
        }
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                !unit.weapon_bonus_enthusiastic,
                "sold tower must removeAllInfluence (clear ENTHUSIASTIC)"
            );
            assert!(!unit.weapon_bonus_subliminal);
            assert!(
                (unit.health.current - hp_after_pulse).abs() < 0.01,
                "sold tower must not keep healing"
            );
        }
    }

    #[test]
    fn listening_outpost_is_not_a_propaganda_heal_aura() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);
        let mut outpost = ThingTemplate::new("ChinaVehicleListeningOutpost");
        outpost
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        logic
            .templates
            .insert("ChinaVehicleListeningOutpost".into(), outpost);
        let _out = logic
            .create_object(
                "ChinaVehicleListeningOutpost",
                Team::China,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("outpost");
        let unit_id = logic
            .create_object("TestInfantry", Team::China, Vec3::new(20.0, 0.0, 0.0))
            .expect("unit");
        {
            let unit = logic.host_object_mut(unit_id).expect("unit");
            let _ = unit.take_damage(40.0);
        }
        let before = logic.host_object(unit_id).expect("unit").health.current;
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        let unit = logic.host_object(unit_id).expect("unit");
        assert!(!unit.weapon_bonus_enthusiastic);
        assert!((unit.health.current - before).abs() < 0.01);
    }

    #[test]
    fn propaganda_scan_delay_lags_enter_and_leave() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);
        let _tower = logic
            .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let unit_id = logic
            .create_object("TestInfantry", Team::China, Vec3::new(250.0, 0.0, 0.0))
            .expect("unit");
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            !logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic
        );

        logic
            .host_object_mut(unit_id)
            .unwrap()
            .set_position(Vec3::new(20.0, 0.0, 0.0));
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            !logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic,
            "enter waits for next 2s scan"
        );

        logic.frame +=
            crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES;
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic
        );

        logic
            .host_object_mut(unit_id)
            .unwrap()
            .set_position(Vec3::new(300.0, 0.0, 0.0));
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic,
            "leave keeps buff until next scan"
        );
        logic.frame +=
            crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES;
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            !logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic
        );
    }

    #[test]
    fn emperor_in_helix_does_not_pulse() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);
        let mut emp = ThingTemplate::new("Tank_ChinaTankEmperor");
        emp.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_health(1100.0);
        logic.templates.insert("Tank_ChinaTankEmperor".into(), emp);
        let mut helix = ThingTemplate::new("ChinaVehicleHelix");
        helix.add_kind_of(KindOf::Vehicle).set_health(300.0);
        logic.templates.insert("ChinaVehicleHelix".into(), helix);

        let helix_id = logic
            .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("helix");
        let emp_id = logic
            .create_object(
                "Tank_ChinaTankEmperor",
                Team::China,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("emperor");
        logic.host_object_mut(emp_id).unwrap().contained_by = Some(helix_id);
        let unit_id = logic
            .create_object("TestInfantry", Team::China, Vec3::new(15.0, 0.0, 0.0))
            .expect("unit");
        {
            let unit = logic.host_object_mut(unit_id).unwrap();
            let _ = unit.take_damage(30.0);
        }
        let before = logic.host_object(unit_id).unwrap().health.current;
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        let unit = logic.host_object(unit_id).unwrap();
        assert!(!unit.weapon_bonus_enthusiastic);
        assert!((unit.health.current - before).abs() < 0.01);
    }

    #[test]
    fn propaganda_allows_allied_players() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);
        let mut china = Player::new(1, Team::China, "China", true);
        china.alliance_team = 7;
        let mut usa = Player::new(2, Team::USA, "USA", false);
        usa.alliance_team = 7;
        logic.add_player(china);
        logic.add_player(usa);

        let _tower = logic
            .create_object_for_player("ChinaSpeakerTower", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let unit_id = logic
            .create_object_for_player("TestInfantry", 2, Vec3::new(20.0, 0.0, 0.0))
            .expect("ally");
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            logic
                .host_object(unit_id)
                .unwrap()
                .weapon_bonus_enthusiastic,
            "ALLOW_ALLIES must buff a co-op teammate"
        );
    }

    #[test]
    fn propaganda_skips_drone_and_unarmed_dozer_rof() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);
        let mut drone = ThingTemplate::new("AmericaScoutDrone");
        drone
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Drone)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0);
        logic.templates.insert("AmericaScoutDrone".into(), drone);
        let mut dozer = ThingTemplate::new("TestDozer");
        dozer
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .add_kind_of(KindOf::Score)
            .set_health(300.0);
        logic.templates.insert("TestDozer".into(), dozer);

        let _tower = logic
            .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let drone_id = logic
            .create_object("AmericaScoutDrone", Team::China, Vec3::new(10.0, 0.0, 0.0))
            .expect("drone");
        let dozer_id = logic
            .create_object("TestDozer", Team::China, Vec3::new(15.0, 0.0, 0.0))
            .expect("dozer");
        {
            let d = logic.host_object_mut(dozer_id).unwrap();
            let _ = d.take_damage(80.0);
        }
        let dozer_before = logic.host_object(dozer_id).unwrap().health.current;
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        assert!(
            !logic
                .host_object(drone_id)
                .unwrap()
                .weapon_bonus_enthusiastic,
            "drone without SCORE is not in the apply set"
        );
        let dozer = logic.host_object(dozer_id).unwrap();
        assert!(
            !dozer.weapon_bonus_enthusiastic,
            "unarmed dozer heals but gets no ROF flags"
        );
        assert!(dozer.health.current > dozer_before);
    }

    #[test]
    fn stealthed_propaganda_pulse_fx_hidden_from_enemies() {
        use crate::game_logic::host_propaganda::should_play_propaganda_pulse_fx;
        assert!(!should_play_propaganda_pulse_fx(
            false, true, false, false, false, false
        ));
        assert!(!should_play_propaganda_pulse_fx(
            false, false, false, true, true, false
        ));
        assert!(should_play_propaganda_pulse_fx(
            true, true, false, true, true, false
        ));
    }

    /// C++ PropagandaTowerBehavior.cpp:450-456 doFXObj on the tower object.
    #[test]
    fn propaganda_pulse_fx_dispatches_at_host_object() {
        let src = include_str!("gps_and_fields.rs");
        let start = src.find("if do_fx {").expect("pulse do_fx");
        let body = &src[start..start + 450];
        assert!(
            body.contains("dispatch_fx_list_at_host_object(fx, tower.id, None)"),
            "PulseFX must use doFXObj on the tower, not doFXPos at sea level"
        );
        assert!(
            !body.contains("dispatch_fx_list_at_pos"),
            "PulseFX must not sit at (x, 0, z): {body}"
        );
    }
    /// C++ PropagandaTowerBehavior.cpp:275 getControllingPlayer()->hasUpgradeComplete.
    /// Same-faction teammate upgrade must not key SUBLIMINAL for another owner.
    #[test]
    fn propaganda_subliminal_uses_controlling_player_not_first_same_team() {
        let mut logic = GameLogic::new();
        insert_speaker_and_infantry(&mut logic);

        let mut teammate = Player::new(3, Team::China, "ChinaA", true);
        teammate.add_completed_upgrade(
            crate::game_logic::host_propaganda::UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
        );
        logic.add_player(teammate);
        logic.add_player(Player::new(7, Team::China, "ChinaB", false));

        let tower_id = logic
            .create_object_for_player("ChinaSpeakerTower", 7, Vec3::new(0.0, 0.0, 0.0))
            .expect("tower");
        let unit_id = logic
            .create_object_for_player("TestInfantry", 7, Vec3::new(20.0, 0.0, 0.0))
            .expect("unit");
        assert_eq!(
            logic.objects.get(&tower_id).and_then(|o| o.owner_player_id),
            Some(7)
        );

        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                unit.weapon_bonus_enthusiastic,
                "owner without upgrade still grants base ENTHUSIASTIC"
            );
            assert!(
                !unit.weapon_bonus_subliminal,
                "teammate SubliminalMessaging must not upgrade another player's tower"
            );
        }

        // Controlling player completes the upgrade → SUBLIMINAL applies.
        if let Some(owner) = logic.get_player_mut(7) {
            owner.add_completed_upgrade(
                crate::game_logic::host_propaganda::UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
            );
        }
        logic.update_propaganda_tower_pulse(1.0 / 30.0);
        {
            let unit = logic.host_object(unit_id).expect("unit");
            assert!(
                unit.weapon_bonus_subliminal,
                "controlling player's hasUpgradeComplete must grant SUBLIMINAL"
            );
        }
    }

    /// C++ ThingTemplate.cpp:384-409 — VEHICLE/INFANTRY inherit default
    /// StealthUpdate; ImmuneToGPS (AIRCRAFT) does not. GrantStealthBehavior.cpp:170
    /// receiveGrant() then flashAsSelected.
    #[test]
    fn gps_grants_default_stealth_to_vehicle_infantry() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(2, Player::new(2, Team::GLA, "GLA", true));

        let mut paladin = ThingTemplate::new("AmericaTankPaladin");
        paladin.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic.templates.insert("AmericaTankPaladin".into(), paladin);
        let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut pathfinder = ThingTemplate::new("AmericaInfantryPathfinder");
        pathfinder.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryPathfinder".into(), pathfinder);
        let mut raptor = ThingTemplate::new("AmericaJetRaptor");
        raptor
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .set_health(160.0);
        logic.templates.insert("AmericaJetRaptor".into(), raptor);

        let pal = logic
            .create_object("AmericaTankPaladin", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
            .expect("paladin");
        let rng = logic
            .create_object("AmericaInfantryRanger", Team::GLA, Vec3::new(8.0, 0.0, 0.0))
            .expect("ranger");
        let pf = logic
            .create_object(
                "AmericaInfantryPathfinder",
                Team::GLA,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("pathfinder");
        let jet = logic
            .create_object("AmericaJetRaptor", Team::GLA, Vec3::new(6.0, 0.0, 0.0))
            .expect("raptor");
        if let Some(o) = logic.host_object_mut(pf) {
            o.innate_stealth = true;
        }

        assert!(
            logic.host_object(pal).unwrap().has_gps_stealth_module(),
            "plain Paladin inherits default StealthUpdate"
        );
        assert!(
            !logic.host_object(pal).unwrap().innate_stealth,
            "default module is not InnateStealth"
        );
        assert!(
            !logic.host_object(jet).unwrap().has_gps_stealth_module(),
            "AIRCRAFT is ImmuneToGPS even when also VEHICLE"
        );

        assert!(logic.activate_gps_scrambler(2, Vec3::ZERO, Some(pal)));
        assert!(
            logic.host_object(pal).unwrap().is_effectively_stealthed(),
            "Paladin default StealthUpdate receives receiveGrant"
        );
        assert!(
            logic.host_object(rng).unwrap().is_effectively_stealthed(),
            "Ranger default StealthUpdate receives receiveGrant"
        );
        assert!(
            logic.host_object(pf).unwrap().is_effectively_stealthed(),
            "Pathfinder innate stealth module receives receiveGrant"
        );
        assert!(
            !logic.host_object(jet).unwrap().is_effectively_stealthed(),
            "Raptor ImmuneToGPS stays visible"
        );
        assert_eq!(
            logic.host_object(pf).unwrap().selection_flash_remaining,
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES,
            "granted Pathfinder must flashAsSelected"
        );
        assert_eq!(
            logic.host_object(pal).unwrap().selection_flash_remaining,
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES,
            "granted Paladin must flashAsSelected"
        );
        assert_eq!(
            logic.host_object(jet).unwrap().selection_flash_remaining,
            0,
            "ungranted Raptor must not flash"
        );
        assert!(
            logic.host_object(pal).unwrap().innate_stealth,
            "receiveGrant sets CAN_STEALTH / innate_stealth"
        );
        assert!(logic.host_object(rng).unwrap().innate_stealth);
    }

    /// C++ ThingTemplate.cpp:391 KINDOF_MOB_NEXUS is ImmuneToGPS — no default
    /// StealthUpdate, so GrantStealthBehavior.cpp:170-173 skips receiveGrant.
    #[test]
    fn gps_does_not_cloak_angry_mob_nexus() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(2, Player::new(2, Team::GLA, "GLA", true));

        let mut nexus = ThingTemplate::new("GLAInfantryAngryMobNexus");
        nexus
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(99999.0);
        logic
            .templates
            .insert("GLAInfantryAngryMobNexus".into(), nexus);
        let mut rebel = ThingTemplate::new("GLAInfantryRebel");
        rebel.add_kind_of(KindOf::Infantry).set_health(120.0);
        logic.templates.insert("GLAInfantryRebel".into(), rebel);

        let nid = logic
            .create_object(
                "GLAInfantryAngryMobNexus",
                Team::GLA,
                Vec3::new(5.0, 0.0, 0.0),
            )
            .expect("nexus");
        let rid = logic
            .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(8.0, 0.0, 0.0))
            .expect("rebel");

        assert!(
            logic.host_object(nid).unwrap().is_kind_of(KindOf::MobNexus),
            "nexus spawn must stamp KINDOF_MOB_NEXUS"
        );
        assert!(
            !logic.host_object(nid).unwrap().has_gps_stealth_module(),
            "MOB_NEXUS ImmuneToGPS strips default StealthUpdate"
        );
        assert!(
            logic.host_object(rid).unwrap().has_gps_stealth_module(),
            "plain Rebel still inherits default StealthUpdate"
        );

        assert!(logic.activate_gps_scrambler(2, Vec3::ZERO, Some(nid)));
        assert!(
            !logic.host_object(nid).unwrap().is_effectively_stealthed(),
            "Angry Mob nexus must stay visible under GPS Scrambler"
        );
        assert!(
            logic.host_object(rid).unwrap().is_effectively_stealthed(),
            "nearby Rebel still receives GPS grant"
        );
        assert_eq!(
            logic.host_object(nid).unwrap().selection_flash_remaining,
            0,
            "ungranted nexus must not flashAsSelected"
        );
    }

    /// C++ StealthUpdate.cpp:184-185 receiveGrant skips only canDisguise()
    /// (Bomb Truck). Hijacker InnateStealth must receive GPS grant immediately.
    #[test]
    fn gps_recloaks_destalthed_hijacker() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(2, Player::new(2, Team::GLA, "GLA", true));

        let mut hijacker = ThingTemplate::new("GLAInfantryHijacker");
        hijacker.add_kind_of(KindOf::Infantry).set_health(120.0);
        logic
            .templates
            .insert("GLAInfantryHijacker".into(), hijacker);

        let hid = logic
            .create_object("GLAInfantryHijacker", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
            .expect("hijacker");
        {
            let h = logic.host_object_mut(hid).expect("hijacker mut");
            assert!(h.innate_stealth, "Hijacker spawn stamps InnateStealth");
            h.set_status_stealthed(false);
            h.stealth_delay_pending = true;
            h.stealth_allowed_frame = 75;
        }

        assert!(
            logic.host_object(hid).unwrap().has_gps_stealth_module(),
            "Hijacker authored StealthUpdate qualifies for receiveGrant"
        );
        assert!(
            !crate::game_logic::host_gps_scrambler::is_gps_scrambler_disguise_name(
                "GLAInfantryHijacker"
            ),
            "Hijacker is not canDisguise"
        );

        assert!(logic.activate_gps_scrambler(2, Vec3::ZERO, Some(hid)));
        assert!(
            logic.host_object(hid).unwrap().is_effectively_stealthed(),
            "GPS must recloak a destalthed Hijacker"
        );
        assert_eq!(
            logic.host_object(hid).unwrap().selection_flash_remaining,
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES,
            "granted Hijacker must flashAsSelected"
        );
    }

    /// C++ GrantStealthBehavior ALLOW_ALLIES — mixed-faction coop teammate.
    #[test]
    fn gps_grants_allied_other_faction_not_same_faction_enemy() {
        let mut logic = GameLogic::new();
        let mut gla = Player::new(2, Team::GLA, "GLA", true);
        gla.alliance_team = 7;
        logic.players.insert(2, gla);
        let mut usa = Player::new(0, Team::USA, "USA", false);
        usa.alliance_team = 7;
        logic.players.insert(0, usa);
        let mut china = Player::new(1, Team::China, "China", false);
        china.alliance_team = 9;
        logic.players.insert(1, china);

        let mut scorp = ThingTemplate::new("GLATankScorpion");
        scorp.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic.templates.insert("GLATankScorpion".into(), scorp);
        let mut crusader = ThingTemplate::new("AmericaTankCrusader");
        crusader.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic
            .templates
            .insert("AmericaTankCrusader".into(), crusader);
        let mut battlemaster = ThingTemplate::new("ChinaTankBattleMaster");
        battlemaster.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic
            .templates
            .insert("ChinaTankBattleMaster".into(), battlemaster);

        let caster = logic
            .create_object("GLATankScorpion", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("caster");
        let ally = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("ally");
        let enemy = logic
            .create_object(
                "ChinaTankBattleMaster",
                Team::China,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("enemy");
        for id in [caster, ally, enemy] {
            if let Some(o) = logic.host_object_mut(id) {
                o.innate_stealth = true;
            }
        }
        if let Some(o) = logic.host_object_mut(caster) {
            o.owner_player_id = Some(2);
        }
        if let Some(o) = logic.host_object_mut(ally) {
            o.owner_player_id = Some(0);
        }
        if let Some(o) = logic.host_object_mut(enemy) {
            o.owner_player_id = Some(1);
        }

        assert!(logic.activate_gps_scrambler(2, Vec3::ZERO, Some(caster)));
        assert!(
            logic.host_object(ally).unwrap().is_effectively_stealthed(),
            "allied USA tank must receive GPS grant"
        );
        assert!(
            !logic.host_object(enemy).unwrap().is_effectively_stealthed(),
            "enemy China tank must not receive GPS grant"
        );
        assert_eq!(
            logic.host_object(ally).unwrap().selection_flash_remaining,
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES,
            "C++ grantStealthToObject flashes granted units"
        );
        assert_eq!(
            logic.host_object(enemy).unwrap().selection_flash_remaining,
            0,
            "ungranted enemy must not flashAsSelected"
        );
    }

    /// C++ ECMTankVehicleDisabler: infantry and aircraft are not targets.
    #[test]
    fn ecm_does_not_jam_infantry_or_aircraft() {
        let mut logic = GameLogic::new();
        let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
        ecm_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);

        let mut inf_tpl = ThingTemplate::new("AmericaInfantryRanger");
        inf_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(20.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".to_string(), inf_tpl);

        let mut air_tpl = ThingTemplate::new("AmericaJetRaptor");
        air_tpl
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(20.0);
        logic
            .templates
            .insert("AmericaJetRaptor".to_string(), air_tpl);

        let _ecm = logic
            .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let inf = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(20.0, 0.0, 0.0),
            )
            .unwrap();
        let air = logic
            .create_object("AmericaJetRaptor", Team::USA, Vec3::new(25.0, 0.0, 0.0))
            .unwrap();
        for id in [inf, air] {
            let o = logic.host_object_mut(id).unwrap();
            o.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                last_fire_time: -5.0,
                ..Weapon::default()
            });
            o.health.current = 20.0;
            o.health.maximum = 20.0;
            o.max_health = 20.0;
        }
        logic.frame = 0;
        logic.update_ecm_jam_field();
        assert!(
            !logic.host_object(inf).unwrap().is_weapons_jammed(),
            "C++ ECMTankVehicleDisabler does not jam infantry"
        );
        assert!(
            !logic.host_object(inf).unwrap().is_subdued_disabled(),
            "infantry must not get DISABLED_SUBDUED from ECM"
        );
        assert!(
            !logic.host_object(air).unwrap().is_weapons_jammed(),
            "C++ ECMTankVehicleDisabler does not jam aircraft"
        );
        assert!(
            !logic.host_object(air).unwrap().is_subdued_disabled(),
            "aircraft must not get DISABLED_SUBDUED from ECM"
        );
    }

    /// C++ ActiveBody.cpp:1292 — 400 HP tank needs many 24-damage pulses.
    #[test]
    fn ecm_vehicle_subdual_accumulates_not_instant() {
        use crate::game_logic::host_ecm_jam::ECM_VEHICLE_DISABLER_DELAY_FRAMES;

        let mut logic = GameLogic::new();
        let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
        ecm_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);
        let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
        tank_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(400.0);
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), tank_tpl);

        let _ecm = logic
            .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let tank = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        {
            let o = logic.host_object_mut(tank).unwrap();
            o.weapon = Some(Weapon {
                damage: 25.0,
                range: 150.0,
                last_fire_time: -5.0,
                ..Weapon::default()
            });
        }
        logic.frame = 0;
        logic.update_ecm_jam_field();
        let after_one = logic.host_object(tank).unwrap();
        assert!(
            !after_one.is_weapons_jammed() && !after_one.is_subdued_disabled(),
            "one 24 pulse must not jam a 400 HP tank"
        );
        assert!(
            after_one.subdual_damage > 0.0,
            "SUBDUAL_VEHICLE must accumulate"
        );
        assert!(
            (after_one.health.current - 400.0).abs() < 1e-3,
            "subdual must not deal HP"
        );

        for i in 1..20 {
            logic.frame = i * ECM_VEHICLE_DISABLER_DELAY_FRAMES;
            logic.update_ecm_jam_field();
        }
        let tank_o = logic.host_object(tank).unwrap();
        assert!(
            tank_o.is_weapons_jammed(),
            "tank weapons_jammed after subdual >= maxHealth (fire-only)"
        );
        assert!(
            tank_o.is_subdued_disabled(),
            "C++ DISABLED_SUBDUED after isSubdued"
        );
        assert!(
            !tank_o.can_move(),
            "DISABLED_SUBDUED skips AI/locomotor updates"
        );
    }

    /// C++ ECMTankVehicleDisabler AttackRange 200 (missile-jammer splash is 150).
    #[test]
    fn ecm_vehicle_disabler_uses_attack_range_200() {
        let mut logic = GameLogic::new();
        let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
        ecm_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);
        let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
        tank_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(400.0);
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), tank_tpl);
        let mut far_tpl = ThingTemplate::new("AmericaTankPaladin");
        far_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(400.0);
        logic
            .templates
            .insert("AmericaTankPaladin".to_string(), far_tpl);

        let _ecm = logic
            .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let near = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(175.0, 0.0, 0.0))
            .unwrap();
        let far = logic
            .create_object("AmericaTankPaladin", Team::USA, Vec3::new(220.0, 0.0, 0.0))
            .unwrap();
        logic.frame = 0;
        logic.update_ecm_jam_field();
        let near_o = logic.host_object(near).unwrap();
        assert!(
            near_o.subdual_damage > 0.0,
            "175u is inside AttackRange 200"
        );
        assert!(
            (near_o.health.current - 400.0).abs() < 1e-3,
            "subdual must not deal HP"
        );
        assert!(!near_o.is_weapons_jammed());
        let far_o = logic.host_object(far).unwrap();
        assert!(
            far_o.subdual_damage.abs() < 1e-3,
            "220u is outside AttackRange 200"
        );
    }

    /// C++ MicrowaveTankBuildingDisabler: 1000 HP building needs many 50 pulses.
    #[test]
    fn microwave_building_subdual_accumulates_not_instant() {
        use crate::game_logic::host_microwave::HOST_MICROWAVE_DELAY_FRAMES;

        let mut logic = GameLogic::new();
        let mut mw_tpl = ThingTemplate::new("AmericaTankMicrowave");
        mw_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic
            .templates
            .insert("AmericaTankMicrowave".to_string(), mw_tpl);
        let mut b_tpl = ThingTemplate::new("ChinaWarFactory");
        b_tpl
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0);
        logic.templates.insert("ChinaWarFactory".to_string(), b_tpl);

        let mw = logic
            .create_object("AmericaTankMicrowave", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let bldg = logic
            .create_object("ChinaWarFactory", Team::China, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        {
            let o = logic.host_object_mut(mw).unwrap();
            o.status.attacking = true;
            o.target = Some(bldg);
        }
        {
            let b = logic.host_object_mut(bldg).unwrap();
            b.object_type = ObjectType::Building;
        }
        logic.frame = 0;
        logic.update_microwave_disable();
        let after_one = logic.host_object(bldg).unwrap();
        assert!(
            !after_one.is_subdued_disabled(),
            "one 50 pulse must not disable a 1000 HP factory"
        );
        assert!(after_one.subdual_damage > 0.0);
        assert!((after_one.health.current - 1000.0).abs() < 1e-3);

        for i in 1..24 {
            logic.frame = i * HOST_MICROWAVE_DELAY_FRAMES;
            logic.update_microwave_disable();
        }
        assert!(
            logic.host_object(bldg).unwrap().is_subdued_disabled(),
            "factory disables after subdual >= maxHealth"
        );
    }

    /// C++ Damage.h:63 MICROWAVE through TankArmor 0% — tanks do not cook.
    #[test]
    fn microwave_emitter_does_not_cook_tanks() {
        use crate::game_logic::host_microwave::{
            HOST_MICROWAVE_EMITTER_DAMAGE, HOST_MICROWAVE_EMITTER_DELAY_FRAMES,
        };

        let mut logic = GameLogic::new();
        let mut mw_tpl = ThingTemplate::new("AmericaTankMicrowave");
        mw_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic
            .templates
            .insert("AmericaTankMicrowave".to_string(), mw_tpl);
        let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
        tank_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(400.0);
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), tank_tpl);
        let mut inf_tpl = ThingTemplate::new("ChinaInfantryRedguard");
        inf_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("ChinaInfantryRedguard".to_string(), inf_tpl);

        let _mw = logic
            .create_object("AmericaTankMicrowave", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let tank = logic
            .create_object(
                "AmericaTankCrusader",
                Team::China,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .unwrap();
        let inf = logic
            .create_object(
                "ChinaInfantryRedguard",
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            )
            .unwrap();
        logic.frame = HOST_MICROWAVE_EMITTER_DELAY_FRAMES;
        let tank_hp = logic.host_object(tank).unwrap().health.current;
        let inf_hp = logic.host_object(inf).unwrap().health.current;
        logic.update_microwave_emitter_field();
        assert!(
            (logic.host_object(tank).unwrap().health.current - tank_hp).abs() < 1e-3,
            "TankArmor MICROWAVE 0% — crusader must not take Unresistable cook"
        );
        let inf_after = logic.host_object(inf).unwrap().health.current;
        assert!(
            (inf_hp - inf_after - HOST_MICROWAVE_EMITTER_DAMAGE).abs() < 0.1,
            "HumanArmor MICROWAVE 100% — infantry take 8"
        );
    }

    /// C++ MicrowaveTankEmitterWeapon RadiusDamageAffects ENEMIES NOT_AIRBORNE.
    #[test]
    fn microwave_emitter_does_not_cook_neutrals() {
        use crate::game_logic::host_microwave::{
            HOST_MICROWAVE_EMITTER_DAMAGE, HOST_MICROWAVE_EMITTER_DELAY_FRAMES,
        };

        let mut logic = GameLogic::new();
        let mut mw_tpl = ThingTemplate::new("AmericaTankMicrowave");
        mw_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0);
        logic
            .templates
            .insert("AmericaTankMicrowave".to_string(), mw_tpl);
        let mut civ_tpl = ThingTemplate::new("CivilianInfantry");
        civ_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(50.0);
        logic
            .templates
            .insert("CivilianInfantry".to_string(), civ_tpl);
        let mut enemy_tpl = ThingTemplate::new("ChinaInfantryRedguard");
        enemy_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("ChinaInfantryRedguard".to_string(), enemy_tpl);

        let _mw = logic
            .create_object("AmericaTankMicrowave", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let civ = logic
            .create_object("CivilianInfantry", Team::Neutral, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        let enemy = logic
            .create_object(
                "ChinaInfantryRedguard",
                Team::China,
                Vec3::new(25.0, 0.0, 0.0),
            )
            .unwrap();
        logic.frame = HOST_MICROWAVE_EMITTER_DELAY_FRAMES;
        let civ_hp = logic.host_object(civ).unwrap().health.current;
        let enemy_hp = logic.host_object(enemy).unwrap().health.current;
        logic.update_microwave_emitter_field();
        assert!(
            (logic.host_object(civ).unwrap().health.current - civ_hp).abs() < 1e-3,
            "emitter RadiusDamageAffects ENEMIES only — neutrals must not cook"
        );
        let enemy_after = logic.host_object(enemy).unwrap().health.current;
        assert!(
            (enemy_hp - enemy_after - HOST_MICROWAVE_EMITTER_DAMAGE).abs() < 0.1,
            "enemy infantry still take MICROWAVE 8"
        );
    }

    /// C++ DumbProjectileBehavior::projectileNowJammed is empty.
    #[test]
    fn ecm_does_not_jam_dumb_tank_shells() {
        let mut logic = GameLogic::new();
        let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
        ecm_tpl.add_kind_of(KindOf::Vehicle).set_health(300.0);
        logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);
        let mut tech_tpl = ThingTemplate::new("GLAVehicleTechnical");
        tech_tpl.add_kind_of(KindOf::Vehicle).set_health(180.0);
        logic
            .templates
            .insert("GLAVehicleTechnical".to_string(), tech_tpl);

        let _ecm = logic
            .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let tech = logic
            .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        {
            let t = logic.host_object_mut(tech).unwrap();
            t.apply_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE");
            logic.apply_technical_weapon_tier(
                tech,
                crate::game_logic::host_technical::TechnicalWeaponTier::One,
            );
        }
        let shell = logic
            .spawn_technical_cannon_shell_projectile(
                tech,
                Vec3::new(10.0, 5.0, 0.0),
                Vec3::new(200.0, 0.0, 0.0),
                None,
            )
            .expect("shell");
        if let Some(o) = logic.objects.get_mut(&shell) {
            o.set_position(Vec3::new(10.0, 5.0, 0.0));
        }
        let aim_before = logic
            .objects
            .get(&shell)
            .and_then(|o| o.technical_cannon_shell_aim)
            .expect("aim");
        logic.update_ecm_missile_jam();
        let o = logic.objects.get(&shell).expect("shell");
        assert!(!o.ecm_missile_jammed, "dumb shells are not jammed");
        let aim_after = o.technical_cannon_shell_aim.expect("aim after");
        assert_eq!(aim_before, aim_after);
        assert_eq!(logic.ecm_missiles_jammed, 0);
    }

    /// C++ FireWeaponUpdate ExclusiveWeaponDelay: disabler shot stamps last_fire_frame
    /// so ECMTankMissileJammer stays silent while the beam is up.
    #[test]
    fn ecm_jammer_honors_exclusive_weapon_delay_while_disabler_fires() {
        use crate::game_logic::host_ecm_jam::{
            ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES, ECM_VEHICLE_DISABLER_DELAY_FRAMES,
            ecm_exclusive_weapon_delay_blocks,
        };

        let mut logic = GameLogic::new();
        let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
        ecm_tpl.add_kind_of(KindOf::Vehicle).set_health(300.0);
        logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);
        let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
        tank_tpl.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), tank_tpl);
        let mut tom_tpl = ThingTemplate::new("AmericaVehicleTomahawk");
        tom_tpl.add_kind_of(KindOf::Vehicle).set_health(180.0);
        logic
            .templates
            .insert("AmericaVehicleTomahawk".to_string(), tom_tpl);

        let ecm = logic
            .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let _enemy = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        let tom = logic
            .create_object(
                "AmericaVehicleTomahawk",
                Team::USA,
                Vec3::new(80.0, 0.0, 0.0),
            )
            .unwrap();
        let missile = logic
            .spawn_tomahawk_missile_projectile(
                tom,
                Vec3::new(10.0, 5.0, 0.0),
                Vec3::new(200.0, 0.0, 0.0),
                None,
            )
            .expect("missile");
        if let Some(o) = logic.objects.get_mut(&missile) {
            o.set_position(Vec3::new(10.0, 5.0, 0.0));
        }

        logic.frame = ECM_VEHICLE_DISABLER_DELAY_FRAMES;
        logic.update_ecm_jam_field();
        let last = logic.host_object(ecm).unwrap().last_fire_frame;
        assert_eq!(last, logic.frame, "disabler pulse stamps last_fire_frame");
        assert!(ecm_exclusive_weapon_delay_blocks(logic.frame, last));

        logic.update_ecm_missile_jam();
        assert!(
            !logic.objects.get(&missile).unwrap().ecm_missile_jammed,
            "jammer must stay silent while ExclusiveWeaponDelay is live"
        );

        logic.frame = last.saturating_add(ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES);
        if let Some(o) = logic.objects.get_mut(&missile) {
            o.ecm_missile_jammed = false;
        }
        logic.update_ecm_missile_jam();
        assert!(
            logic.objects.get(&missile).unwrap().ecm_missile_jammed,
            "jammer pulses again after ExclusiveWeaponDelay"
        );
    }
}
