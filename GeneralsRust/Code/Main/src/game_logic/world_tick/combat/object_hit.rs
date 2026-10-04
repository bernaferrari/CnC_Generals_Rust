//! Accepted object-target effects. Their original branch/callback order is
//! deliberately preserved; this structural split does not normalize residuals.
use super::*;

impl GameLogic {
    pub(super) fn apply_combat_object_hit(
        &mut self,
        attacker_id: ObjectId,
        attacker_team: Team,
        target_id: ObjectId,
        target_position: Vec3,
        slot: u8,
        enemy_or_forced: bool,
        mut weapon_damage: f32,
    ) {
        // Aurora dive bomb residual: queue delayed area damage at target.
        // AuroraBomb projectile flight residual closed; FuelAir gas OCL path.
        // Instant single-target take_damage is skipped; AOE applies after delay.
        // Keep fired_slot so last_fire_time / particles / audio still run.
        let aurora_queued = {
            use crate::game_logic::host_aurora_bomb::{
                aurora_bomb_kind_for_template, is_aurora_aircraft_template,
            };
            let aurora = self.objects.get(&attacker_id).and_then(|a| {
                if is_aurora_aircraft_template(&a.template_name) {
                    Some(aurora_bomb_kind_for_template(&a.template_name))
                } else {
                    None
                }
            });
            if let Some(kind) = aurora {
                let impact = target_position;
                let _ = self.queue_aurora_bomb(kind, attacker_id, attacker_team, impact);
                true
            } else {
                false
            }
        };

        if aurora_queued {
            // Shot consumed; delayed dive residual pending (no instant HP damage).
        } else {
            // Avenger Target Designator residual: paint FAERIE_FIRE (no HP damage).
            let avenger_paint = {
                use crate::game_logic::host_avenger::{
                    AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO, is_avenger_template,
                    should_apply_faerie_fire_paint,
                };
                self.objects
                    .get(&attacker_id)
                    .map(|a| {
                        should_apply_faerie_fire_paint(
                            is_avenger_template(&a.template_name),
                            slot,
                            true,
                            enemy_or_forced,
                        )
                    })
                    .unwrap_or(false)
            };
            let avenger_air = {
                use crate::game_logic::host_avenger::{
                    is_avenger_template, should_apply_avenger_air_laser,
                };
                let target_is_air = self
                    .objects
                    .get(&target_id)
                    .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                    .unwrap_or(false);
                self.objects
                    .get(&attacker_id)
                    .map(|a| {
                        should_apply_avenger_air_laser(
                            is_avenger_template(&a.template_name),
                            slot,
                            target_is_air,
                            true,
                            enemy_or_forced,
                        )
                    })
                    .unwrap_or(false)
            };
            // Humvee air TOW residual damage boost vs aircraft.
            let humvee_air_tow = {
                use crate::game_logic::host_humvee::{
                    HUMVEE_AIR_TOW_DAMAGE, humvee_prefer_air_tow, is_humvee_template,
                };
                let target_is_air = self
                    .objects
                    .get(&target_id)
                    .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                    .unwrap_or(false);
                self.objects
                    .get(&attacker_id)
                    .map(|a| {
                        humvee_prefer_air_tow(
                            is_humvee_template(&a.template_name),
                            a.has_upgrade_tag(
                                crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW,
                            ) || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                            target_is_air,
                        ) && slot == 1
                    })
                    .unwrap_or(false)
            };
            if humvee_air_tow {
                weapon_damage = crate::game_logic::host_humvee::HUMVEE_AIR_TOW_DAMAGE;
            }

            // TARGET_FAERIE_FIRE ROF honesty when shooting a painted target.
            if self
                .objects
                .get(&target_id)
                .map(|t| t.is_faerie_fire())
                .unwrap_or(false)
            {
                self.avenger.record_rof_grant();
            }

            if avenger_paint {
                use crate::game_logic::host_avenger::{
                    AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO,
                };
                let until = self
                    .frame
                    .saturating_add(AVENGER_FAERIE_FIRE_DURATION_FRAMES);
                if let Some(target) = self.objects.get_mut(&target_id) {
                    target.apply_faerie_fire(until);
                }
                self.avenger.record_paint();
                let muzzle = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| a.get_position())
                    .unwrap_or(target_position);
                self.queue_audio_event(
                    AudioEventRequest::new(AVENGER_PAINT_AUDIO)
                        .with_object(attacker_id)
                        .with_position(muzzle)
                        .with_priority(140),
                );
                // Status residual: no hitpoint damage from designator.
            } else if avenger_air {
                self.avenger.record_air_laser_fire();
                if let Some(target) = self.objects.get_mut(&target_id) {
                    let destroyed = target.take_damage_from(weapon_damage, Some(attacker_id));
                    if destroyed {
                        let victim_pos = target.get_position();
                        let victim_team = target.team;
                        self.mark_object_for_destruction(target_id, Some(attacker_team));
                        let wname = self
                            .objects
                            .get(&attacker_id)
                            .and_then(|a| a.weapon_name_for_slot(slot).map(str::to_owned));
                        self.continue_or_stop_after_kill(
                            attacker_id,
                            target_id,
                            victim_pos,
                            victim_team,
                            wname.as_deref(),
                            20.0,
                        );
                    }
                }
            } else {
                // Nuke Cannon primary residual: area shell + medium radiation field.
                let nuke_primary = {
                    use crate::game_logic::host_nuke_cannon::{
                        is_nuke_cannon_template, should_apply_nuke_cannon_primary,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_nuke_cannon_primary(
                                is_nuke_cannon_template(&a.template_name),
                                slot,
                            )
                        })
                        .unwrap_or(false)
                };

                // Neutron shell residual: Nuke Cannon secondary applies blast
                // (kill infantry / unman vehicles) instead of HP take_damage.
                let neutron_blast = {
                    use crate::game_logic::host_neutron_shell::{
                        UPGRADE_CHINA_NEUTRON_SHELLS, is_nuke_cannon_template,
                        should_apply_neutron_blast,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_neutron_blast(
                                a.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                                    || a.has_upgrade_tag("Upgrade_ChinaNeutronShells"),
                                slot,
                                is_nuke_cannon_template(&a.template_name),
                            )
                        })
                        .unwrap_or(false)
                };

                if nuke_primary {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_nuke_cannon_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        // Honesty fire count is recorded on impact via apply;
                        // count residual fire at spawn for combat-gate honesty.
                        (1, false)
                    } else {
                        self.apply_nuke_cannon_primary_at(impact, Some(attacker_id), attacker_team)
                    };
                } else if neutron_blast {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_neutron_cannon_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (ik, vu, _vk) = if spawned {
                        // Blast deferred to shell DetonateCallsKill residual.
                        self.neutron_shell_residual_blasts =
                            self.neutron_shell_residual_blasts.saturating_add(1);
                        (0, 0, 0)
                    } else {
                        self.apply_neutron_blast_at(impact, attacker_team, Some(attacker_id), true)
                    };
                    // Stop attack after residual blast shot (slow reload residual).
                } else if {
                    // Helix residual: PRIMARY HelixMinigunWeapon intended-only.
                    // When portable gattling addon is installed, the Overlord/Helix
                    // gattling residual path already applies primary + passenger.
                    use crate::game_logic::host_helix_minigun::should_apply_helix_minigun_residual;
                    use crate::game_logic::host_overlord_addons::is_helix_template;
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_helix_minigun_residual(
                                is_helix_template(&a.template_name),
                                slot,
                            ) && !a.has_overlord_gattling_residual()
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let (hits, _destroyed_any) = self.apply_helix_minigun_residual_at(
                        impact,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // Comanche residual: 20mm primary / anti-tank secondary /
                    // manual rocket pods tertiary.
                    use crate::game_logic::host_comanche_rocket_pods::{
                        UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template,
                        should_apply_comanche_antitank_residual,
                        should_apply_comanche_cannon_residual, should_apply_rocket_pod_area_attack,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| is_comanche_template(&a.template_name))
                        .unwrap_or(false)
                } {
                    use crate::game_logic::host_comanche_rocket_pods::{
                        UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template,
                        should_apply_comanche_antitank_residual,
                        should_apply_comanche_cannon_residual, should_apply_rocket_pod_area_attack,
                    };
                    let impact = target_position;
                    let (has_pods, is_comanche) = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| {
                            (
                                a.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                                    || a.has_upgrade_tag("Upgrade_ComancheRocketPods"),
                                is_comanche_template(&a.template_name),
                            )
                        })
                        .unwrap_or((false, false));
                    let (hits, _destroyed_any) =
                        if should_apply_rocket_pod_area_attack(is_comanche, has_pods, slot) {
                            {
                                use crate::game_logic::host_comanche_rocket_pods::{
                                    ROCKET_POD_CLIP_SIZE, rocket_pod_scatter_impact,
                                };
                                let idx = {
                                    let shot = self
                                        .comanche_rocket_pod_shot_index
                                        .entry(attacker_id)
                                        .or_insert(0);
                                    let i = *shot;
                                    *shot = shot.saturating_add(1) % ROCKET_POD_CLIP_SIZE.max(1);
                                    i
                                };
                                let (sx, sy, sz) =
                                    rocket_pod_scatter_impact(impact.x, impact.y, impact.z, idx);
                                let aim = Vec3::new(sx, sy, sz);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|o| o.get_position())
                                    .unwrap_or(impact);
                                let _ = self.spawn_comanche_rocket_pod_projectile(
                                    attacker_id,
                                    from,
                                    aim,
                                    idx,
                                );
                                // Area residual still centers on intended aim
                                // (ScatterTarget is projectile flight residual).
                                self.apply_comanche_rocket_pod_area_at(impact, Some(attacker_id))
                            }
                        } else if should_apply_comanche_antitank_residual(
                            is_comanche,
                            slot,
                            has_pods,
                        ) {
                            self.apply_comanche_antitank_residual_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                            )
                        } else if should_apply_comanche_cannon_residual(is_comanche, slot) {
                            self.apply_comanche_cannon_residual_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                            )
                        } else {
                            (0, false)
                        };
                } else if {
                    // GLA Rocket Buggy residual: long-range rocket + splash / scatter.
                    use crate::game_logic::host_rocket_buggy::{
                        is_rocket_buggy_template, should_apply_rocket_buggy_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_rocket_buggy_residual(is_rocket_buggy_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_rocket_buggy_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.rocket_buggy_residual_fires =
                            self.rocket_buggy_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_rocket_buggy_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // GLA SCUD launcher residual: area blast (+ toxin field on secondary).
                    use crate::game_logic::host_scud_launcher::{
                        is_scud_launcher_template, should_apply_scud_area,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_scud_area(is_scud_launcher_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let toxin = {
                        use crate::game_logic::host_scud_launcher::scud_toxin_warhead_for_slot;
                        self.objects
                            .get(&attacker_id)
                            .map(|a| scud_toxin_warhead_for_slot(&a.template_name, slot))
                            .unwrap_or(slot == 1)
                    };
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_scud_launcher_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            toxin,
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        (1, false) // fire residual honesty; blast deferred to impact
                    } else {
                        self.apply_scud_area_at(impact, Some(attacker_id), attacker_team, toxin)
                    };
                } else if {
                    // GLA Technical residual: MG direct or cannon/RPG splash salvage tiers.
                    use crate::game_logic::host_technical::{
                        TechnicalWeaponTier, is_technical_template, should_apply_technical_splash,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            if !is_technical_template(&a.template_name) {
                                return false;
                            }
                            let tier = Self::technical_tier_from_object(a);
                            // Always apply residual path for technical (MG direct or splash).
                            let _ = should_apply_technical_splash(true, tier);
                            true
                        })
                        .unwrap_or(false)
                } {
                    use crate::game_logic::host_technical::{
                        TechnicalWeaponTier as TechTier, should_apply_technical_cannon_shell,
                        should_apply_technical_rpg_missile,
                    };
                    let impact = target_position;
                    let tier = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| Self::technical_tier_from_object(a))
                        .unwrap_or(TechTier::Base);
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let (hits, _destroyed_any) = if should_apply_technical_rpg_missile(true, tier) {
                        let spawned = self
                            .spawn_technical_rpg_missile_projectile(
                                attacker_id,
                                from,
                                impact,
                                Some(target_id),
                            )
                            .is_some();
                        if spawned {
                            self.technical_residual_fires =
                                self.technical_residual_fires.saturating_add(1);
                            (1, false)
                        } else {
                            self.apply_technical_residual_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                            )
                        }
                    } else if should_apply_technical_cannon_shell(true, tier) {
                        let spawned = self
                            .spawn_technical_cannon_shell_projectile(
                                attacker_id,
                                from,
                                impact,
                                Some(target_id),
                            )
                            .is_some();
                        if spawned {
                            self.technical_residual_fires =
                                self.technical_residual_fires.saturating_add(1);
                            (1, false)
                        } else {
                            self.apply_technical_residual_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                            )
                        }
                    } else {
                        self.apply_technical_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // GLA Marauder residual: salvage fire-rate tiers + small splash.
                    use crate::game_logic::host_marauder::{
                        is_marauder_template, should_apply_marauder_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_marauder_residual(is_marauder_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    use crate::game_logic::host_marauder::{
                        MARAUDER_SPEED_TIER0, MARAUDER_SPEED_TIER1, MARAUDER_SPEED_TIER2,
                        MarauderWeaponTier,
                    };
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let speed = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| match Self::marauder_tier_from_object(a) {
                            MarauderWeaponTier::Two => MARAUDER_SPEED_TIER2,
                            MarauderWeaponTier::One => MARAUDER_SPEED_TIER1,
                            MarauderWeaponTier::Base => a
                                .weapon
                                .as_ref()
                                .map(|w| w.projectile_speed)
                                .filter(|s| *s > 1.0)
                                .unwrap_or(MARAUDER_SPEED_TIER0),
                        })
                        .unwrap_or(MARAUDER_SPEED_TIER0);
                    let spawned = self
                        .spawn_marauder_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            speed,
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.marauder_residual_fires =
                            self.marauder_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_marauder_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // GLA Scorpion residual: gun splash or rocket dual-radius secondary.
                    use crate::game_logic::host_scorpion::{
                        is_scorpion_template, should_apply_scorpion_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_scorpion_residual(is_scorpion_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = if slot == 0 {
                        self.spawn_scorpion_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            slot,
                        )
                        .is_some()
                    } else {
                        self.spawn_scorpion_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            slot,
                        )
                        .is_some()
                    };
                    let (hits, _destroyed_any) = if spawned {
                        if slot == 0 {
                            self.scorpion_residual_fires =
                                self.scorpion_residual_fires.saturating_add(1);
                        } else {
                            self.scorpion_residual_missile_fires =
                                self.scorpion_residual_missile_fires.saturating_add(1);
                        }
                        (1, false)
                    } else {
                        self.apply_scorpion_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                            slot,
                        )
                    };
                } else if {
                    // USA Tomahawk residual: dual-radius long-range missile.
                    use crate::game_logic::host_tomahawk::{
                        is_tomahawk_template, should_apply_tomahawk_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_tomahawk_residual(is_tomahawk_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_tomahawk_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.tomahawk_residual_fires =
                            self.tomahawk_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_tomahawk_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // USA Raptor residual: jet missiles + Laser Missiles splash.
                    use crate::game_logic::host_raptor::{
                        is_raptor_template, should_apply_raptor_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| should_apply_raptor_residual(is_raptor_template(&a.template_name)))
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_raptor_missile_projectile(attacker_id, from, impact, Some(target_id))
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.raptor_residual_fires = self.raptor_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_raptor_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // China MiG residual: dual-radius napalm / Nuke missiles + field residual.
                    use crate::game_logic::host_mig::{is_mig_template, should_apply_mig_residual};
                    self.objects
                        .get(&attacker_id)
                        .map(|a| should_apply_mig_residual(is_mig_template(&a.template_name)))
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_mig_missile_projectile(attacker_id, from, impact, Some(target_id))
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.mig_residual_fires = self.mig_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_mig_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // America Fire Base residual: howitzer primary-radius splash.
                    use crate::game_logic::host_fire_base::{
                        is_fire_base_template, should_apply_fire_base_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_fire_base_residual(is_fire_base_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_fire_base_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.fire_base_residual_fires =
                            self.fire_base_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_fire_base_residual_at(impact, Some(attacker_id), Some(target_id))
                    };
                } else if {
                    // USA Stealth Fighter residual: jet missiles splash + bunker-buster structure path.
                    use crate::game_logic::host_stealth_fighter::{
                        is_stealth_fighter_template, should_apply_stealth_fighter_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_stealth_fighter_residual(is_stealth_fighter_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_stealth_jet_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.stealth_fighter_residual_fires =
                            self.stealth_fighter_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_stealth_fighter_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // USA Battle Drone residual: intended-only MG fire.
                    use crate::game_logic::host_slave_drones::is_battle_drone_template;
                    self.objects
                        .get(&attacker_id)
                        .map(|a| is_battle_drone_template(&a.template_name))
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let (hits, _destroyed_any) = self.apply_battle_drone_residual_at(
                        impact,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // China Overlord / Emperor residual: dual-radius main gun (no gattling addon).
                    use crate::game_logic::host_overlord_gun::{
                        is_overlord_gun_chassis, should_apply_overlord_gun_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_overlord_gun_residual(
                                is_overlord_gun_chassis(&a.template_name),
                                a.has_overlord_gattling_residual(),
                            )
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_overlord_shell_projectile(attacker_id, from, impact, Some(target_id))
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        (1, false)
                    } else {
                        self.apply_overlord_gun_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // GLA Jarmen Kell residual: primary sniper (intended-only).
                    use crate::game_logic::host_jarmen_kell::{
                        is_jarmen_kell_template, should_apply_jarmen_kell_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_jarmen_kell_residual(is_jarmen_kell_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let (hits, _destroyed_any) = self.apply_jarmen_kell_residual_at(
                        impact,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // USA Crusader/Paladin residual: GenericTankShell Bezier + splash.
                    use crate::game_logic::host_usa_tanks::{
                        CRUSADER_WEAPON_SPEED, PALADIN_WEAPON_SPEED, is_paladin_template,
                        should_apply_usa_tank_gun_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| should_apply_usa_tank_gun_residual(&a.template_name))
                        .unwrap_or(false)
                } {
                    use crate::game_logic::host_usa_tanks::{
                        CRUSADER_WEAPON_SPEED, PALADIN_WEAPON_SPEED, is_paladin_template,
                    };
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let (speed, is_pal) = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| {
                            let pal = is_paladin_template(&a.template_name);
                            let spd = a
                                .weapon
                                .as_ref()
                                .map(|w| w.projectile_speed)
                                .filter(|s| *s > 1.0)
                                .unwrap_or(if pal {
                                    PALADIN_WEAPON_SPEED
                                } else {
                                    CRUSADER_WEAPON_SPEED
                                });
                            (spd, pal)
                        })
                        .unwrap_or((CRUSADER_WEAPON_SPEED, false));
                    let _ = is_pal;
                    let spawned = self
                        .spawn_usa_tank_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            speed,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        (1, false)
                    } else {
                        self.apply_usa_tank_gun_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        if hits > 0 {
                            if let Some(w) = attacker.weapon.as_mut() {
                                w.last_fire_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                            }
                        }
                    }
                    let _ = hits;
                } else if {
                    // China Battlemaster residual: tank gun splash + Uranium damage residual.
                    use crate::game_logic::host_battlemaster::{
                        is_battlemaster_template, should_apply_battlemaster_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_battlemaster_residual(is_battlemaster_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_battlemaster_shell_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        (1, false)
                    } else {
                        self.apply_battlemaster_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // China Tank Hunter residual: RPG splash + AA capable residual.
                    use crate::game_logic::host_tank_hunter::{
                        is_tank_hunter_template, should_apply_tank_hunter_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_tank_hunter_residual(is_tank_hunter_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_tank_hunter_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.tank_hunter_residual_fires =
                            self.tank_hunter_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_tank_hunter_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // China Red Guard residual: bayonet one-shot vs close infantry, else gun.
                    use crate::game_logic::host_red_guard::{
                        is_red_guard_template, should_apply_red_guard_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_red_guard_residual(is_red_guard_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let (hits, _destroyed_any) = self.apply_red_guard_residual_at(
                        target_position,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // GLA RPG Trooper residual: rocket splash + AA capable residual.
                    use crate::game_logic::host_rpg_trooper::{
                        is_rpg_trooper_template, should_apply_rpg_trooper_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_rpg_trooper_residual(is_rpg_trooper_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_rpg_trooper_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.rpg_trooper_residual_fires =
                            self.rpg_trooper_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_rpg_trooper_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // GLA Terrorist residual: SuicideDynamitePack self-detonation.
                    use crate::game_logic::host_terrorist::{
                        is_terrorist_template, should_apply_terrorist_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_terrorist_residual(is_terrorist_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let (hits, _destroyed_any) =
                        self.apply_terrorist_residual_at(Some(attacker_id), Some(target_id));
                } else if {
                    // USA Missile Defender residual: missile splash + laser guided secondary.
                    use crate::game_logic::host_missile_defender::{
                        is_missile_defender_template, should_apply_missile_defender_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_missile_defender_residual(is_missile_defender_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let laser_slot = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.active_weapon_slot == 1)
                        .unwrap_or(false);
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_missile_defender_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            laser_slot,
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        if laser_slot {
                            self.missile_defender_residual_laser_fires =
                                self.missile_defender_residual_laser_fires.saturating_add(1);
                        } else {
                            self.missile_defender_residual_fires =
                                self.missile_defender_residual_fires.saturating_add(1);
                        }
                        (1, false)
                    } else {
                        self.apply_missile_defender_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                            laser_slot,
                        )
                    };
                } else if {
                    // GLA Rebel residual: machine gun intended-only residual.
                    use crate::game_logic::host_gla_rebel::{
                        is_gla_rebel_template, should_apply_rebel_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_rebel_residual(is_gla_rebel_template(&a.template_name))
                        })
                        .unwrap_or(false)
                } {
                    let (hits, _destroyed_any) = self.apply_rebel_residual_at(
                        target_position,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // USA Ranger residual: rifle intended-only or FlashBang dual-radius splash.
                    use crate::game_logic::host_ranger::{
                        is_ranger_template, should_apply_ranger_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| should_apply_ranger_residual(is_ranger_template(&a.template_name)))
                        .unwrap_or(false)
                } {
                    let flash_slot = slot == 1;
                    let impact = target_position;
                    let (hits, _destroyed_any) = if flash_slot {
                        let from = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| a.get_position())
                            .unwrap_or(impact);
                        let spawned = self
                            .spawn_flashbang_grenade_projectile(
                                attacker_id,
                                from,
                                impact,
                                Some(target_id),
                            )
                            .is_some();
                        if spawned {
                            self.ranger_residual_flashbang_fires =
                                self.ranger_residual_flashbang_fires.saturating_add(1);
                            (1, false)
                        } else {
                            self.apply_ranger_residual_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                                true,
                            )
                        }
                    } else {
                        self.apply_ranger_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                            false,
                        )
                    };
                } else if {
                    // USA Humvee TOW residual: HumveeMissile / PatriotMissile flight + splash.
                    use crate::game_logic::host_humvee::{
                        is_humvee_template, should_apply_humvee_tow_residual,
                    };
                    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
                    let (is_hv, has_tow) = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| {
                            (
                                is_humvee_template(&a.template_name),
                                a.has_upgrade_tag(UPGRADE_AMERICA_TOW)
                                    || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                            )
                        })
                        .unwrap_or((false, false));
                    should_apply_humvee_tow_residual(is_hv, has_tow, slot == 1)
                } {
                    use crate::game_logic::host_humvee::{
                        HUMVEE_TOW_FIRE_AUDIO as HV_TOW_AUDIO, humvee_prefer_air_tow as hv_air_tow,
                    };
                    let impact = target_position;
                    let target_is_air = self
                        .objects
                        .get(&target_id)
                        .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                        .unwrap_or(false);
                    let air = hv_air_tow(true, true, target_is_air);
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_humvee_tow_missile_projectile(
                            attacker_id,
                            from,
                            impact,
                            Some(target_id),
                            air,
                        )
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.humvee_tow_residual_fires =
                            self.humvee_tow_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.humvee_tow_residual_fires =
                            self.humvee_tow_residual_fires.saturating_add(1);
                        self.apply_humvee_tow_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                            air,
                        )
                    };
                    let _ = HV_TOW_AUDIO;
                } else if {
                    // China MiniGunner residual: ground gun or AA secondary hit.
                    use crate::game_logic::host_minigunner::{
                        is_minigunner_template, should_apply_minigunner_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_minigunner_residual(is_minigunner_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let (hits, _destroyed_any) = self.apply_minigunner_residual_at(
                        target_position,
                        Some(attacker_id),
                        Some(target_id),
                        slot,
                    );
                } else if {
                    // Colonel Burton residual: knife one-shot vs close infantry, else sniper.
                    use crate::game_logic::host_colonel_burton::{
                        is_colonel_burton_template, should_apply_burton_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_burton_residual(is_colonel_burton_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let (hits, _destroyed_any) = self.apply_burton_residual_at(
                        target_position,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // China Troop Crawler residual: TroopCrawlerAssault DEPLOY → unload + attack.
                    use crate::game_logic::host_troop_crawler::{
                        is_troop_crawler_template, should_apply_troop_crawler_assault_deploy,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_troop_crawler_assault_deploy(is_troop_crawler_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let _ordered = self.apply_troop_crawler_assault_deploy(attacker_id, target_id);
                    // DEPLOY residual deals no meaningful HP damage (PrimaryDamage ~0).
                } else if {
                    // China Dragon Tank residual: DragonTankFlameProjectile flight + dual-radius splash.
                    use crate::game_logic::host_dragon_tank::{
                        is_dragon_tank_template, should_apply_dragon_flame_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_dragon_flame_residual(is_dragon_tank_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let from = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.get_position())
                        .unwrap_or(impact);
                    let spawned = self
                        .spawn_dragon_flame_projectile(attacker_id, from, impact, Some(target_id))
                        .is_some();
                    let (hits, _destroyed_any) = if spawned {
                        self.dragon_tank_residual_fires =
                            self.dragon_tank_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.dragon_tank_residual_fires =
                            self.dragon_tank_residual_fires.saturating_add(1);
                        self.apply_dragon_flame_residual_at(
                            impact,
                            Some(attacker_id),
                            Some(target_id),
                        )
                    };
                } else if {
                    // China Gattling Tank residual: ground gun or AA secondary hit.
                    use crate::game_logic::host_gattling_tank::is_gattling_tank_template;
                    self.objects
                        .get(&attacker_id)
                        .map(|a| is_gattling_tank_template(&a.template_name))
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let (hits, _destroyed_any) = self.apply_gattling_tank_residual_at(
                        impact,
                        Some(attacker_id),
                        Some(target_id),
                        slot,
                    );
                } else if {
                    // Portable gattling is independent (try_overlord_gattling_addon_independent_fire).
                    // Do not piggyback stacked +10 onto the host chassis shot.
                    false
                } {
                    let _ = slot;
                } else if {
                    // GLA Combat Cycle residual: rider weapon fire / suicide residual.
                    use crate::game_logic::host_combat_cycle::{
                        is_combat_cycle_template, should_apply_combat_cycle_residual,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            should_apply_combat_cycle_residual(is_combat_cycle_template(
                                &a.template_name,
                            ))
                        })
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let (hits, _destroyed_any) = self.apply_combat_cycle_residual_at(
                        impact,
                        Some(attacker_id),
                        Some(target_id),
                    );
                } else if {
                    // GLA Toxin Tractor residual: poison stream primary or contaminate spray.
                    use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;
                    self.objects
                        .get(&attacker_id)
                        .map(|a| is_toxin_tractor_template(&a.template_name))
                        .unwrap_or(false)
                } {
                    let impact = target_position;
                    let spray = slot == 1;
                    let (hits, _destroyed_any) = if spray {
                        self.apply_toxin_tractor_spray_at(impact, Some(attacker_id), attacker_team)
                    } else {
                        let from = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| a.get_position())
                            .unwrap_or(impact);
                        let spawned = self
                            .spawn_toxin_stream_projectile(
                                attacker_id,
                                from,
                                impact,
                                Some(target_id),
                            )
                            .is_some();
                        if spawned {
                            (1, false)
                        } else {
                            self.apply_toxin_tractor_stream_at(
                                impact,
                                Some(attacker_id),
                                Some(target_id),
                                attacker_team,
                            )
                        }
                    };
                } else {
                    // Bunker Buster residual: kill garrisoned occupants + amplify bunker damage.
                    // KILL_GARRISONED residual: leftover store DamageType only
                    // (C++ ActiveBody.cpp:421-460). AllowAttackGarrisonedBldgs is
                    // estimate-only and must not skip structure HP.
                    let (bunker_buster_hit, kill_garrisoned_hit) = {
                        use crate::game_logic::host_bunker_buster::{
                            UPGRADE_AMERICA_BUNKER_BUSTERS, is_bunker_buster_carrier,
                            should_apply_bunker_buster, should_apply_kill_garrisoned,
                        };
                        let target_is_structure = self
                            .objects
                            .get(&target_id)
                            .map(|t| t.is_kind_of(KindOf::Structure))
                            .unwrap_or(false);
                        self.objects
                            .get(&attacker_id)
                            .map(|a| {
                                let has_upgrade = a
                                    .has_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS)
                                    || a.has_upgrade_tag(
                                        "Upgrade_AmericaBunkerBusters",
                                    );
                                let carrier =
                                    is_bunker_buster_carrier(&a.template_name);
                                let kill_garrisoned = a
                                    .weapon_name_for_slot(slot)
                                    .map(crate::game_logic::weapon_bootstrap::host_weapon_is_kill_garrisoned_damage)
                                    .unwrap_or(false);
                                (
                                    should_apply_bunker_buster(
                                        has_upgrade,
                                        carrier,
                                        target_is_structure,
                                    ),
                                    should_apply_kill_garrisoned(
                                        kill_garrisoned,
                                        target_is_structure,
                                    ),
                                )
                            })
                            .unwrap_or((false, false))
                    };

                    if bunker_buster_hit {
                        let (_kills, _structure_dmg, destroyed) = self
                            .apply_bunker_buster_to_target(
                                target_id,
                                attacker_team,
                                weapon_damage,
                                Some(attacker_id),
                            );
                        if destroyed {
                            self.stop_attack_decision_aware(attacker_id);
                        }
                    } else if kill_garrisoned_hit {
                        let _kills = self.apply_kill_garrisoned_to_target(
                            target_id,
                            attacker_team,
                            weapon_damage,
                            Some(attacker_id),
                        );
                    } else if {
                        let table_offset =
                            self.objects.get_mut(&attacker_id).and_then(|attacker| {
                                let name = attacker.weapon_name_for_slot(slot).map(str::to_owned);
                                attacker.take_scatter_table_offset(slot, name.as_deref())
                            });
                        let (sc_miss, sc_impact, sc_splash) = self.resolve_instant_scatter_shot(
                            attacker_id,
                            target_id,
                            slot,
                            target_position,
                            table_offset,
                        );
                        if sc_miss {
                            // C++ ScatterRadius residual: miss intended; splash at offset.
                            if sc_splash > 0.0 {
                                let wname_splash =
                                    self.objects.get(&attacker_id).and_then(|attacker| {
                                        attacker.weapon_name_for_slot(slot).map(str::to_owned)
                                    });
                                let hits = self.apply_scatter_miss_splash_at(
                                    sc_impact,
                                    weapon_damage,
                                    sc_splash,
                                    attacker_id,
                                    attacker_team,
                                    target_id,
                                    wname_splash.as_deref(),
                                );
                            }
                            true
                        } else {
                            false
                        }
                    } {
                        // Miss path handled above (splash optional).
                    } else {
                        // C++ Weapon.cpp:1378-1380 dealDamage copies
                        // WeaponTemplate DamageType/DeathType onto DamageInfo.
                        // take_damage_from is UNRESISTABLE (script kill /
                        // empty-hulk); live object-vs-object fire must use
                        // the firing Weapon.ini type so Armor.ini applies.
                        let fire_wname = self.objects.get(&attacker_id).and_then(|attacker| {
                            attacker.weapon_name_for_slot(slot).map(str::to_owned)
                        });
                        let damage_type = fire_wname.as_deref().map(
                            crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name,
                        )
                        // C++ WeaponTemplate ctor defaults m_damageType
                        // to DAMAGE_EXPLOSION (Weapon.cpp:249); an
                        // unnamed host weapon fires Explosion, not Bullet.
                        .unwrap_or(crate::game_logic::combat::DamageType::Explosive);
                        let death_type =
                            crate::game_logic::host_armor_residual::resolve_host_death_type(
                                fire_wname.as_deref(),
                                damage_type,
                            );
                        crate::game_logic::object::prime_live_damage_context(
                            self.objects.get(&attacker_id),
                            fire_wname.as_deref(),
                            damage_type,
                        );
                        let at_self = fire_wname.as_deref().map(
                            crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name,
                        )
                        .unwrap_or(false);
                        let shooter_pos = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| a.get_position())
                            .unwrap_or(target_position);
                        if !at_self {
                            if let Some(target) = self.objects.get_mut(&target_id) {
                                if target.get_sneaky_targeting_offset(self.frame).is_some() {
                                    // C++ fireWeaponTemplate clears victimObj; the shot
                                    // flies at the offset point and does not connect.
                                } else {
                                    let destroyed = target.take_damage_from_typed_death(
                                        weapon_damage,
                                        Some(attacker_id),
                                        damage_type,
                                        death_type,
                                    );
                                    if destroyed {
                                        // C++ parity: XP is victim ExperienceValue at current level.
                                        let kill_xp = target.kill_experience_value();
                                        let victim_pos = target.get_position();
                                        let victim_team = target.team;
                                        self.mark_object_for_destruction(
                                            target_id,
                                            Some(attacker_team),
                                        );
                                        self.continue_or_stop_after_kill(
                                            attacker_id,
                                            target_id,
                                            victim_pos,
                                            victim_team,
                                            fire_wname.as_deref(),
                                            kill_xp,
                                        );
                                    }
                                }
                            }
                        }
                        // C++ dual-radius splash residual after direct hit.
                        // DamageDealtAtSelfPosition recenters on the shooter and
                        // clears primary-victim skip (Weapon.cpp:1008, 1035).
                        {
                            use crate::game_logic::weapon_bootstrap::{
                                host_primary_damage_radius_for_weapon_name,
                                host_secondary_damage_for_weapon_name,
                                host_secondary_damage_radius_for_weapon_name,
                            };
                            let wname = self.objects.get(&attacker_id).and_then(|attacker| {
                                attacker.weapon_name_for_slot(slot).map(str::to_owned)
                            });
                            let (pr, sr, sd) = if let Some(ref n) = wname {
                                (
                                    host_primary_damage_radius_for_weapon_name(n),
                                    host_secondary_damage_radius_for_weapon_name(n),
                                    host_secondary_damage_for_weapon_name(n),
                                )
                            } else {
                                (0.0, 0.0, 0.0)
                            };
                            let (splash_weapon, radius_mult) = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| {
                                    (
                                        a.weapon_slot(slot)
                                            .map(|w| w.splash_radius.max(0.0))
                                            .unwrap_or(0.0),
                                        a.weapon_bonus_radius(),
                                    )
                                })
                                .unwrap_or((0.0, 1.0));
                            // C++ getPrimary/SecondaryDamageRadius — RADIUS field.
                            let primary_r =
                                (if pr > 0.0 { pr } else { splash_weapon }) * radius_mult;
                            let secondary_r = sr * radius_mult;
                            if primary_r > 0.0 || secondary_r > 0.0 {
                                let sec_dmg = sd;
                                let splash_pos = if at_self {
                                    shooter_pos
                                } else {
                                    target_position
                                };
                                let splash_intended = if at_self { ObjectId(0) } else { target_id };
                                let hits = self.apply_instant_hit_splash_at(
                                    splash_pos,
                                    weapon_damage,
                                    sec_dmg,
                                    primary_r,
                                    secondary_r,
                                    attacker_id,
                                    attacker_team,
                                    splash_intended,
                                    wname.as_deref(),
                                );
                            }
                        }
                    }
                }
            } // end !avenger_paint / !avenger_air residual branch
        } // end !aurora_queued

        // Inferno Cannon residual: InfernoTankShell Bezier flight → FireFieldSmall.
        // Fail-closed: instant FireFieldSmall zone if shell spawn fails.
        // Skipped for Aurora (delayed dive residual already queued).
        if !aurora_queued {
            use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
            let is_inferno = self
                .objects
                .get(&attacker_id)
                .map(|a| is_inferno_cannon_template(&a.template_name))
                .unwrap_or(false);
            if is_inferno {
                let impact = target_position;
                let upgraded = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_inferno_cannon::has_black_napalm_upgrade(
                            &a.applied_upgrades,
                        )
                    })
                    .unwrap_or(false);
                let from = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| a.get_position())
                    .unwrap_or(impact);
                let spawned = self
                    .spawn_inferno_shell_projectile(
                        attacker_id,
                        from,
                        impact,
                        Some(target_id),
                        upgraded,
                    )
                    .is_some();
                if !spawned {
                    let _ =
                        self.spawn_inferno_fire_zone(attacker_id, attacker_team, impact, upgraded);
                }
            }
        }
    }
}
