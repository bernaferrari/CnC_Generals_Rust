//! Successful-shot bookkeeping, visual/audio/OCL dispatch and ammo/barrels.
//! CPP Weapon.cpp:2617-2669 and FiringTracker.cpp:60 onward are the contract;
//! current Main residual ordering remains unchanged by this extraction.
use super::*;

impl GameLogic {
    pub(super) fn commit_combat_shot(
        &mut self,
        input: CombatAttackInput,
        slot: u8,
        visual: Option<super::super::super::world_combat::AcceptedWeaponDischarge>,
    ) {
        self.commit_combat_shot_with_effects(input, slot, visual, &mut Default::default())
    }

    pub(super) fn commit_combat_shot_with_effects(
        &mut self,
        input: CombatAttackInput,
        slot: u8,
        visual: Option<super::super::super::world_combat::AcceptedWeaponDischarge>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let CombatAttackInput {
            attacker_id,
            current_time,
            target_id,
            target_location,
            ..
        } = input;
        let native_rule = self
            .objects
            .get(&attacker_id)
            .and_then(|source| source.selected_native_weapon_rule(slot));
        let native_rank = |rank| match rank {
            VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
            VeterancyLevel::Veteran => gamelogic::common::VeterancyLevel::Veteran,
            VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
            VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
        };
        let target_has_faerie_fire = target_id
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|target| target.is_faerie_fire());
        let possible_next_shot_frame =
            self.objects
                .get(&attacker_id)
                .map_or(self.frame, |attacker| {
                    attacker.possible_next_shot_frame_for_slot_with_solo_set(
                        slot,
                        self.frame,
                        target_has_faerie_fire,
                        &self.solo_weapon_bonus_set,
                    )
                });
        // Pathfinder residual sniper honesty (any successful fire from residual).
        if self
            .objects
            .get(&attacker_id)
            .map(|a| crate::game_logic::host_pathfinder::is_pathfinder_template(&a.template_name))
            .unwrap_or(false)
        {
            self.pathfinder_residual_sniper_fires =
                self.pathfinder_residual_sniper_fires.saturating_add(1);
        }

        // Quad Cannon residual honesty: ground primary vs AA secondary fires.
        if self
            .objects
            .get(&attacker_id)
            .map(|a| crate::game_logic::host_quad_cannon::is_quad_cannon_template(&a.template_name))
            .unwrap_or(false)
        {
            if slot == 1 {
                self.quad_cannon_residual_aa_fires =
                    self.quad_cannon_residual_aa_fires.saturating_add(1);
            } else {
                self.quad_cannon_residual_ground_fires =
                    self.quad_cannon_residual_ground_fires.saturating_add(1);
            }
        }

        // China Gattling Tank residual: advance continuous-fire ramp + honesty.
        if self
            .objects
            .get(&attacker_id)
            .map(|a| {
                crate::game_logic::host_gattling_tank::is_gattling_tank_template(&a.template_name)
            })
            .unwrap_or(false)
        {
            self.advance_gattling_continuous_fire(attacker_id, target_id, slot);
        }

        // China MiniGunner residual: advance continuous-fire ramp + honesty.
        if self
            .objects
            .get(&attacker_id)
            .map(|a| crate::game_logic::host_minigunner::is_minigunner_template(&a.template_name))
            .unwrap_or(false)
        {
            self.advance_minigunner_continuous_fire(attacker_id, target_id, slot);
        }

        // A cannon reaching this shared accepted-discharge route still owns
        // one structure FiringTracker transition. The dedicated base-defense
        // driver performs this phase itself and never calls commit_combat_shot.
        if self.objects.get(&attacker_id).is_some_and(|attacker| {
            crate::game_logic::host_base_defense::is_gattling_cannon_structure(
                &attacker.template_name,
            )
        }) {
            self.advance_gattling_building_continuous_fire(attacker_id, target_id, slot);
        }

        // Combat particle residual: weapon fire → muzzle (+ impact) registry entries.
        let muzzle_pos = self
            .objects
            .get(&attacker_id)
            .map(|a| a.get_position())
            .unwrap_or(Vec3::ZERO);
        let fire_target = target_id.filter(|id| self.objects.contains_key(id));
        let impact_pos = fire_target
            .and_then(|id| self.objects.get(&id).map(|t| t.get_position()))
            .or(target_location);
        let fire_frame = self.frame;
        let (fire_fx, det_fx) = {
            self.objects
                .get(&attacker_id)
                .and_then(|attacker| {
                    let veterancy = attacker.experience.level;
                    if let Some(rule) = native_rule.as_ref() {
                        return Some((
                            rule.get_fire_fx(native_rank(veterancy)).map(|fx| fx.name().to_owned()).unwrap_or_default(),
                            rule.get_projectile_detonate_fx(native_rank(veterancy)).map(|fx| fx.name().to_owned()).unwrap_or_default(),
                        ));
                    }
                    attacker.weapon_name_for_slot(slot).map(|weapon_name| {
                        (
                            crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                            crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                        )
                    })
                })
                .unwrap_or_default()
        };
        // C++ Weapon::fireWeaponTemplate FireFX stealth gate residual:
        // stealthed+undetected+non-disguised suppress muzzle FX unless
        // the observer locally controls the source, PlayFXWhenStealthed
        // is set, or the source is KINDOF_MINE.
        let suppress_fire_fx = {
            let a = self.objects.get(&attacker_id);
            a.map(|o| {
                let locally_controlled = source_is_locally_controlled(
                    o.owner_player_id,
                    self.local_player_id(),
                );
                let is_mine = o.is_kind_of(KindOf::Mine);
                let hidden = !locally_controlled
                    && o.status.stealthed
                    && !o.status.detected
                    && !o.status.disguised
                    && !is_mine;
                if !hidden {
                    return false;
                }
                let wname = o.weapon_name_for_slot(slot);
                let play = native_rule.as_ref().map(|rule| rule.play_fx_when_stealthed).unwrap_or_else(|| wname
                    .map(crate::game_logic::weapon_bootstrap::host_play_fx_when_stealthed_for_weapon_name)
                    .unwrap_or(false));
                !play
            })
            .unwrap_or(false)
        };
        // C++ Weapon.cpp:904-939: doFXPos only when FireFX is non-null.
        // Dispatch play_dispatch_fire_fx fail-closes on empty FireFX
        // (TestTank has no Weapon.ini name). Residual muzzle/impact
        // still registers so fire is observable; named FireFX stays
        // on the dispatch path. See combat_fire_fx.rs.
        // A known native null FireFX is silent. The primitive fallback below
        // belongs only to explicitly unadmitted, nameless probes.
        // The accepted definition was sampled before callbacks. A promotion
        // or a receiving-only definition must not create a second generic
        // muzzle after its named FireFX already ran (or was suppressed).
        if native_rule.is_none()
            && self.weapon_definitions.is_none()
            && !visual
                .as_ref()
                .is_some_and(|shot| shot.has_authored_fire_fx)
        {
            self.spawn_residual_muzzle_when_dispatch_has_no_fire_fx(
                suppress_fire_fx,
                &fire_fx,
                &det_fx,
                muzzle_pos,
                impact_pos,
                fire_frame,
                attacker_id,
                fire_target,
            );
        }
        // C++ Weapon.ini LaserName residual: short-lived combat beam for
        // presentation / laser_segment_upload observe path.
        {
            let weapon_name = self
                .objects
                .get(&attacker_id)
                .and_then(|attacker| attacker.weapon_name_for_slot(slot));
            let laser_name = native_rule
                .as_ref()
                .map(|rule| rule.laser_name.clone())
                .unwrap_or_else(|| {
                    weapon_name
                        .map(crate::game_logic::weapon_bootstrap::host_laser_name_for_weapon_name)
                        .unwrap_or_default()
                });
            if !laser_name.is_empty() {
                let laser_bone = native_rule.as_ref().map(|rule| rule.laser_bone_name.clone()).unwrap_or_else(|| weapon_name.map(crate::game_logic::weapon_bootstrap::host_laser_bone_name_for_weapon_name).unwrap_or_default());
                let to = impact_pos.unwrap_or(muzzle_pos);
                let laser_name_owned = laser_name.clone();
                self.weapon_lasers.push(
                    crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
                        laser_name,
                        laser_bone,
                        attacker_id,
                        fire_target,
                        (muzzle_pos.x, muzzle_pos.y, muzzle_pos.z),
                        (to.x, to.y, to.z),
                        fire_frame,
                    ),
                );
                let _ = self.spawn_weapon_laser_beam_object_with_effects(
                    &laser_name_owned,
                    attacker_id,
                    fire_target,
                    muzzle_pos,
                    to,
                    effects,
                );
            }
        }

        // Audio residual (hq-7zxm slice): weapon fire → real AudioEventRequest.
        // C++ FiringTracker::shotFired plays `weaponFired->getFireSound()`
        // (FiringTracker.cpp:144-155) — the WeaponTemplate's authored
        // AudioEventRTS parsed from `FireSound` (Weapon.cpp:171,
        // Weapon.h:678). AudioManager::addAudioEvent returns AHSV_NoSound
        // for an empty event name (GameAudio.cpp:384-386): a weapon with
        // no authored FireSound is silent. Never queue an invented
        // generic token — no "WeaponFire" AudioEvent exists in retail
        // SoundEffects.ini, so it can only dead-end as ERR(no-info).
        let fire_sound = if let Some(rule) = native_rule.as_ref() {
            let name = rule.fire_sound.name();
            (!name.is_empty()).then(|| name.to_owned())
        } else {
            self.objects
                .get(&attacker_id)
                .and_then(|attacker| attacker.weapon_name_for_slot(slot))
                .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
                .filter(|sound| !sound.is_empty())
        };
        if let Some(fire_sound) = fire_sound {
            self.queue_audio_event(
                AudioEventRequest::new(fire_sound.as_str())
                    .with_object(attacker_id)
                    .with_position(muzzle_pos)
                    .with_priority(160),
            );
        }

        // Capture weapon name before mut borrow for RETURN_TO_BASE peels.
        let fire_wname = self
            .objects
            .get(&attacker_id)
            .and_then(|attacker| attacker.weapon_name_for_slot(slot).map(str::to_owned));
        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            let auto_reloaded_clip = if let Some(weapon) = attacker.weapon_slot_mut(slot) {
                Object::consume_ammo_on_fire_named(weapon, current_time, fire_wname.as_deref());
                Object::auto_reloaded_clip_after_firing(weapon, fire_wname.as_deref())
            } else {
                false
            };
            // Match Object::fireCurrentWeapon: only the actual
            // temporarily locked slot can end that lock by finishing
            // an auto-reloading clip.  Do not let an unrelated
            // PRIMARY/SECONDARY fallback discharge a TERTIARY lock.
            if auto_reloaded_clip
                && attacker.weapon_lock_type == WeaponLockType::LockedTemporarily
                && attacker.weapon_lock_slot == slot
            {
                attacker.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }
            // Each lane owns one FiringTracker transition. Specialized lanes
            // already advanced its canonical count/victim and derived ROF;
            // finish only the shared post-shot effects here.
            let specialized_lane = crate::game_logic::host_gattling_tank::is_gattling_tank_template(
                &attacker.template_name,
            ) || crate::game_logic::host_minigunner::is_minigunner_template(
                &attacker.template_name,
            )
                || crate::game_logic::host_base_defense::is_gattling_cannon_structure(
                    &attacker.template_name,
                );
            if specialized_lane {
                attacker.finish_shot_after_specialized_tracker_with_solo_set(
                    &self.solo_weapon_bonus_set,
                );
            } else {
                attacker.record_shot_at_target_with_solo_set(
                    target_id,
                    fire_frame,
                    possible_next_shot_frame,
                    &self.solo_weapon_bonus_set,
                );
            }
            attacker.stamp_auto_reload_when_idle_from_slot(slot, fire_frame);
            // C++ STEALTH_NOT_WHILE_ATTACKING residual: combat fire breaks stealth.
            if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                attacker.break_stealth();
            }
        }
        // This direct normal-combat finalizer does not route through
        // Object::fire_at_ex.  Its accepted shot already consumed
        // ammo above, so normalize the exact pre-advance barrel here
        // rather than leaving this live route cursor-static or
        // synthesizing recoil from a fire-intent writeback.
        if let Some(visual) = visual {
            self.finish_accepted_weapon_discharge(visual);
        } else if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            attacker.advance_weapon_barrel_after_shot(slot);
        }
    }
}
