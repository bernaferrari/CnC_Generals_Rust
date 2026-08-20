//! Host scripts `impl GameLogic` — `stealth_mines`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! stealth and detection / mines / demo traps
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Host residual for C++ StealthUpdate + StealthDetectorUpdate targetability.
    ///
    /// - Expires `OBJECT_STATUS_DETECTED` when `detection_expires_frame` is reached
    /// - Detectors mark nearby enemy stealthed units as detected (hold ~1s)
    /// - Bomb truck disguise reveal residual (RevealDistanceFromTarget = 100)
    /// - Sentry / Camo Rebel / heroes apply StealthDelay before re-cloak
    /// - Stealth Fighter + mines innate cloak; contained detectors do not scan
    pub fn update_stealth_and_detection(&mut self) {
        let frame = self.frame;

        // Expire timed detections (unit may remain stealthed).
        for obj in self.objects.values_mut() {
            if obj.status.detected
                && obj.detection_expires_frame > 0
                && frame >= obj.detection_expires_frame
            {
                obj.clear_detected();
            }
        }

        // C++ StealthUpdate.cpp:365-373 — occupants of non-garrison transports destalth.
        {
            let destalth: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() || !obj.status.stealthed {
                        return None;
                    }
                    let container = self.objects.get(&obj.contained_by?)?;
                    let garrisonable = container.is_garrison_contain()
                        || container.thing.template.garrison_contain_max.is_some();
                    if Object::transport_contain_should_destalth(garrisonable) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for id in destalth {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.break_stealth();
                    if obj.stealth_delay_frames > 0 {
                        obj.stealth_allowed_frame =
                            frame.saturating_add(obj.stealth_delay_frames);
                        obj.stealth_delay_pending = false;
                    }
                }
            }
        }

        // Bomb truck disguise residual: RevealDistanceFromTarget = 100 while
        // attacking a victim; also reveal when firing breaks stealth residual.
        {
            use crate::game_logic::host_bomb_truck_disguise::{
                should_reveal_disguise_by_distance, BOMB_TRUCK_DISGUISE_REVEAL_AUDIO,
            };
            let disguised: Vec<(ObjectId, Option<ObjectId>, bool, Vec3)> = self
                .objects
                .iter()
                .filter(|(_, o)| o.status.disguised && o.is_alive())
                .map(|(id, o)| {
                    (
                        *id,
                        o.target,
                        o.status.attacking
                            || matches!(
                                o.ai_state,
                                AIState::Attacking
                                    | AIState::AttackMoving
                                    | AIState::AttackingGround
                            ),
                        o.get_position(),
                    )
                })
                .collect();
            let mut reveal_ids: Vec<ObjectId> = Vec::new();
            for (id, victim_id, is_attacking, pos) in disguised {
                let mut reveal = false;
                if is_attacking {
                    if let Some(vid) = victim_id {
                        if let Some(victim) = self.objects.get(&vid) {
                            let vp = victim.get_position();
                            let dx = pos.x - vp.x;
                            let dz = pos.z - vp.z;
                            let dist = (dx * dx + dz * dz).sqrt();
                            if should_reveal_disguise_by_distance(dist) {
                                reveal = true;
                            }
                        } else {
                            // Attacking with no live victim: still residual-reveal
                            // when attack state is active (fire residual).
                            reveal = true;
                        }
                    } else {
                        reveal = true;
                    }
                }
                if reveal {
                    reveal_ids.push(id);
                }
            }
            for rid in reveal_ids {
                let pos = {
                    let Some(obj) = self.objects.get_mut(&rid) else {
                        continue;
                    };
                    obj.clear_disguise();
                    obj.get_position()
                };
                self.bomb_truck_disguise.record_reveal();
                self.queue_audio_event(
                    AudioEventRequest::new(BOMB_TRUCK_DISGUISE_REVEAL_AUDIO)
                        .with_object(rid)
                        .with_position(pos)
                        .with_priority(160),
                );
            }
        }

        // Pathfinder residual: StealthForbiddenConditions = MOVING.
        // Uncloak while Moving/AttackMoving; re-cloak immediately when stopped
        // (StealthDelay = 0, InnateStealth = Yes). Fire does not break stealth.
        {
            use crate::game_logic::host_pathfinder::pathfinder_stealth_desired;
            // Class bit set at spawn — no per-frame template-name scan.
            let pf_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_pathfinder_unit && o.is_alive())
                .map(|(id, _)| *id)
                .collect();
            for pid in pf_ids {
                let Some(obj) = self.objects.get_mut(&pid) else {
                    continue;
                };
                let moving = matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                    || obj.status.moving;
                if let Some(desired) = pathfinder_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    moving,
                ) {
                    if desired && !obj.status.stealthed {
                        obj.set_status_stealthed(true);
                    } else if !desired && obj.status.stealthed {
                        obj.break_stealth();
                    }
                }
            }
        }

        // Sentry Drone residual: StealthForbiddenConditions = FIRING_PRIMARY + MOVING,
        // StealthDelay 2000ms (60f) before re-cloak. C++ AmericaVehicleSentryDrone
        // ModuleTag_06 / StealthUpdate.cpp allowedToStealth.
        {
            use crate::game_logic::host_sentry_drone::{
                is_sentry_drone_template, sentry_stealth_allowed_frame, sentry_stealth_desired,
            };
            let sentry_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_alive() && is_sentry_drone_template(&o.template_name))
                .map(|(id, _)| *id)
                .collect();
            for sid in sentry_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame = sentry_stealth_allowed_frame(frame);
                    obj.stealth_delay_pending = false;
                }
                let moving = matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                    || obj.status.moving;
                let firing = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                let Some(desired) = sentry_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.is_alive(),
                    moving,
                    firing,
                    frame,
                    obj.stealth_allowed_frame,
                ) else {
                    continue;
                };
                if desired && !obj.status.stealthed {
                    obj.set_status_stealthed(true);
                    obj.set_status_detected(false);
                    obj.detection_expires_frame = 0;
                    obj.stealth_allowed_frame = 0;
                } else if !desired && obj.status.stealthed {
                    obj.break_stealth();
                    if obj.stealth_delay_pending {
                        obj.stealth_allowed_frame = sentry_stealth_allowed_frame(frame);
                        obj.stealth_delay_pending = false;
                    }
                }
            }
        }

        // Listening Outpost residual: StealthForbiddenConditions = MOVING RIDERS_ATTACKING.
        // InnateStealth re-cloaks when stopped and riders are idle.
        {
            use crate::game_logic::host_listening_outpost::listening_outpost_stealth_desired;
            // Style bit installed at spawn for LO templates — no name scan.
            let lo_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_alive() && o.is_listening_outpost_style_container())
                .map(|(id, _)| *id)
                .collect();
            for lid in lo_ids {
                let occupants = self
                    .objects
                    .get(&lid)
                    .map(|o| o.contained_units())
                    .unwrap_or_default();
                // C++ OpenContain::isAnyRiderAttacking — OBJECT_STATUS_IS_ATTACKING.
                let riders_attacking = occupants.iter().any(|rid| {
                    self.objects.get(rid).is_some_and(|rider| {
                        rider.is_alive()
                            && (rider.status.attacking
                                || matches!(
                                    rider.ai_state,
                                    AIState::Attacking
                                        | AIState::AttackMoving
                                        | AIState::AttackingGround
                                ))
                    })
                });
                let Some(obj) = self.objects.get_mut(&lid) else {
                    continue;
                };
                let moving = matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                    || obj.status.moving;
                if let Some(desired) = listening_outpost_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    moving,
                    riders_attacking,
                ) {
                    if desired && !obj.status.stealthed {
                        obj.set_status_stealthed(true);
                    } else if !desired && obj.status.stealthed {
                        obj.break_stealth();
                    }
                }
            }
        }

        // GLA Camouflage residual: StealthForbiddenConditions = ATTACKING USING_ABILITY,
        // StealthDelay 2500ms (75f) before re-cloak. C++ GLAInfantryRebel StealthUpdate.
        {
            use crate::game_logic::host_upgrades::{
                camouflage_stealth_allowed_frame, camouflage_unit_stealth_desired,
                UPGRADE_GLA_CAMOUFLAGE,
            };
            // Upgrade tag is only applied to camouflage-eligible units at unlock.
            let camo_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && !o.status.disguised
                        && (o.has_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE)
                            || o.has_upgrade_tag("Upgrade_GLACamouflage"))
                })
                .map(|(id, _)| *id)
                .collect();
            for cid in camo_ids {
                let Some(obj) = self.objects.get_mut(&cid) else {
                    continue;
                };
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame = camouflage_stealth_allowed_frame(frame);
                    obj.stealth_delay_pending = false;
                }
                let attacking = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
                let Some(desired) = camouflage_unit_stealth_desired(
                    obj.innate_stealth,
                    obj.is_alive(),
                    attacking,
                    using_ability,
                    frame,
                    obj.stealth_allowed_frame,
                ) else {
                    continue;
                };
                if desired && !obj.status.stealthed {
                    obj.set_status_stealthed(true);
                    obj.set_status_detected(false);
                    obj.detection_expires_frame = 0;
                    obj.stealth_allowed_frame = 0;
                } else if !desired && obj.status.stealthed {
                    obj.break_stealth();
                    if obj.stealth_delay_pending {
                        obj.stealth_allowed_frame = camouflage_stealth_allowed_frame(frame);
                        obj.stealth_delay_pending = false;
                    }
                }
            }
        }

        // GLA CamoNetting structure residual: StealthForbiddenConditions =
        // ATTACKING + USING_ABILITY + TAKING_DAMAGE, StealthDelay 2500ms re-cloak,
        // OrderIdleEnemiesToAttackMeUponReveal residual on uncloak,
        // FriendlyOpacity residual (min cloaked / max revealed + pulse while cloaked),
        // StealthLook / heat-vision second-pass residual (Drawable::setStealthLook).
        {
            use crate::game_logic::host_upgrades::{
                camo_netting_heat_vision_opacity, camo_netting_order_idle_enemy_in_range,
                camo_netting_pulse_opacity, camo_netting_stealth_allowed_frame,
                camo_netting_stealth_look, camo_netting_structure_stealth_desired,
                is_camo_netting_structure_template, CAMO_NETTING_FRIENDLY_OPACITY_MAX,
                CAMO_NETTING_FRIENDLY_OPACITY_MIN, UPGRADE_GLA_CAMO_NETTING,
            };
            let struct_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && is_camo_netting_structure_template(&o.template_name)
                        && (o.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING)
                            || o.has_upgrade_tag("Upgrade_GLACamoNetting")
                            || o.stealth_breaks_on_damage)
                })
                .map(|(id, _)| *id)
                .collect();
            let mut recloaks = 0u32;
            let mut reveals = 0u32;
            let mut opacity_cloaks = 0u32;
            let mut opacity_reveals = 0u32;
            let mut heat_vision = 0u32;
            let mut revealed_ids: Vec<ObjectId> = Vec::new();
            for sid in struct_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                // Resolve pending StealthDelay after a reveal this frame.
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(frame);
                    obj.stealth_delay_pending = false;
                }
                let attacking = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                // C++ OBJECT_STATUS_IS_USING_ABILITY residual.
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
                let Some(desired) = camo_netting_structure_stealth_desired(
                    obj.innate_stealth,
                    obj.is_alive(),
                    attacking,
                    using_ability,
                    frame,
                    obj.stealth_allowed_frame,
                ) else {
                    continue;
                };
                if desired && !obj.status.stealthed {
                    obj.set_status_stealthed(true);
                    obj.set_status_detected(false);
                    obj.detection_expires_frame = 0;
                    obj.stealth_allowed_frame = 0;
                    // FriendlyOpacity residual: cloaked → min (then pulse).
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MIN;
                    obj.record_host_vision_camo();
                    obj.camo_opacity_pulse_phase = 0.0;
                    opacity_cloaks = opacity_cloaks.saturating_add(1);
                    recloaks = recloaks.saturating_add(1);
                } else if !desired && obj.status.stealthed {
                    obj.break_stealth();
                    // break_stealth marks delay pending; resolve immediately with frame.
                    if obj.stealth_delay_pending {
                        obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(frame);
                        obj.stealth_delay_pending = false;
                    }
                    // FriendlyOpacity residual: revealed → max (no pulse).
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;
                    obj.record_host_vision_camo();
                    opacity_reveals = opacity_reveals.saturating_add(1);
                    reveals = reveals.saturating_add(1);
                    revealed_ids.push(sid);
                } else if obj.status.stealthed && !obj.status.detected {
                    // Pulse residual while cloaked (C++ setEffectiveOpacity sin path).
                    // If still at default full opacity (spawned already cloaked),
                    // record one cloak opacity residual.
                    if (obj.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() < 0.01
                        && obj.camo_opacity_pulse_phase == 0.0
                    {
                        opacity_cloaks = opacity_cloaks.saturating_add(1);
                    }
                    let (op, next_phase) = camo_netting_pulse_opacity(obj.camo_opacity_pulse_phase);
                    obj.camo_friendly_opacity = op;
                    obj.record_host_vision_camo();
                    obj.camo_opacity_pulse_phase = next_phase;
                } else {
                    // Revealed residual: hold max opacity.
                    if (obj.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() > 0.01
                    {
                        obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;
                        obj.record_host_vision_camo();
                        opacity_reveals = opacity_reveals.saturating_add(1);
                    }
                }

                // StealthLook residual for enemy observer (default host residual view).
                // Detected stealthed structures → heat-vision second material pass.
                let look = camo_netting_stealth_look(
                    obj.status.stealthed,
                    obj.status.detected,
                    false, // enemy observer residual (non-friendly)
                );
                let hv = camo_netting_heat_vision_opacity(look);
                if hv > 0.0 && obj.camo_heat_vision_opacity < 0.5 {
                    heat_vision = heat_vision.saturating_add(1);
                }
                obj.camo_stealth_look = look.as_u8();
                obj.record_host_vision_camo();
                obj.camo_heat_vision_opacity = hv;
                // CamoNetting sub-object net mesh residual presentation.
                if obj.camo_net_sub_object_shown || obj.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING) {
                    use crate::game_logic::host_upgrades::{
                        camo_netting_sub_object_observer_visible, camo_netting_sub_object_state,
                    };
                    obj.camo_net_sub_object_shown = true;
                    let sub = camo_netting_sub_object_state(
                        true,
                        obj.status.stealthed,
                        obj.status.detected,
                        false, // enemy observer residual default
                        obj.camo_friendly_opacity,
                    );
                    obj.camo_net_sub_object_observer_visible =
                        camo_netting_sub_object_observer_visible(&sub);
                }
            }
            self.camo_netting_heat_vision_count = self
                .camo_netting_heat_vision_count
                .saturating_add(heat_vision);
            self.camo_netting_opacity_cloak_count = self
                .camo_netting_opacity_cloak_count
                .saturating_add(opacity_cloaks);
            self.camo_netting_opacity_reveal_count = self
                .camo_netting_opacity_reveal_count
                .saturating_add(opacity_reveals);
            // OrderIdleEnemiesToAttackMeUponReveal residual: idle enemy units
            // that can see the revealed structure wake and attempt to target it.
            for rid in revealed_ids {
                let Some(victim) = self.objects.get(&rid) else {
                    continue;
                };
                let v_team = victim.team;
                let v_pos = victim.get_position();
                let candidates: Vec<(ObjectId, f32, f32, bool)> = self
                    .objects
                    .iter()
                    .filter(|(_, o)| {
                        o.is_alive()
                            && o.team != v_team
                            && o.team != Team::Neutral
                            && !matches!(
                                o.object_type,
                                ObjectType::Building | ObjectType::Projectile
                            )
                            && !o.is_kind_of(KindOf::Structure)
                            && !o.is_kind_of(KindOf::Worker)
                            && !o.is_worker()
                    })
                    .map(|(id, o)| {
                        let dx = o.get_position().x - v_pos.x;
                        let dz = o.get_position().z - v_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        let vision = {
                            let sr = o.get_template().sight_range;
                            if sr > 0.0 {
                                sr
                            } else {
                                150.0
                            }
                        };
                        let can_attack = o.weapon.is_some()
                            || o.is_kind_of(KindOf::Attackable)
                            || o.can_attack()
                            || matches!(
                                o.object_type,
                                ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft
                            );
                        (*id, dist, vision, can_attack)
                    })
                    .collect();
                for (eid, dist, vision, can_attack) in candidates {
                    let Some(enemy) = self.objects.get_mut(&eid) else {
                        continue;
                    };
                    let idle = matches!(enemy.ai_state, AIState::Idle) && enemy.target.is_none();
                    if !camo_netting_order_idle_enemy_in_range(
                        enemy.is_alive(),
                        idle,
                        can_attack,
                        dist,
                        vision,
                    ) {
                        continue;
                    }
                    enemy.set_target(Some(rid));
                    enemy.set_ai_state(AIState::Attacking);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_attack(eid, rid);
                        crate::game_logic::host_ai_decision_log::record_set_state(eid, 2);
                    }
                    self.camo_netting_order_idle_enemies_count =
                        self.camo_netting_order_idle_enemies_count.saturating_add(1);
                }
            }
            self.camo_netting_structure_residual_recloaks = self
                .camo_netting_structure_residual_recloaks
                .saturating_add(recloaks);
            self.camo_netting_structure_residual_reveals = self
                .camo_netting_structure_residual_reveals
                .saturating_add(reveals);
        }

        // Innate-stealth heroes: Burton / Kell / Lotus / Saboteur / Hijacker.
        // C++ StealthUpdate Forbidden=FIRING_PRIMARY / ATTACKING / USING_ABILITY
        // plus StealthDelay (2000ms Burton/Kell, 2500ms Lotus/Saboteur).
        {
            use crate::game_logic::host_colonel_burton::{
                burton_stealth_desired, is_colonel_burton_template,
            };
            use crate::game_logic::host_hero_abilities::{
                is_black_lotus_template, lotus_stealth_desired,
            };
            use crate::game_logic::host_jarmen_kell::{
                is_jarmen_kell_template, jarmen_stealth_desired,
            };
            use crate::game_logic::host_radar_stealth_vision_residual::hero_stealth_delay_frames_residual;
            let hero_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && (is_colonel_burton_template(&o.template_name)
                            || is_jarmen_kell_template(&o.template_name)
                            || is_black_lotus_template(&o.template_name)
                            || o.template_name.to_ascii_lowercase().contains("saboteur")
                            || o.template_name.to_ascii_lowercase().contains("hijacker"))
                })
                .map(|(id, _)| *id)
                .collect();
            for hid in hero_ids {
                let Some(obj) = self.objects.get_mut(&hid) else {
                    continue;
                };
                if obj.stealth_delay_frames == 0 {
                    obj.stealth_delay_frames =
                        hero_stealth_delay_frames_residual(&obj.template_name);
                }
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame =
                        frame.saturating_add(obj.stealth_delay_frames);
                    obj.stealth_delay_pending = false;
                }
                let firing = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                // C++ FIRING_PRIMARY only — Burton knife (secondary) does not destalth.
                let firing_primary = firing
                    && obj.last_fire_slot == 0
                    && obj.last_fire_sim_time > 0.0;
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
                let desired = if is_colonel_burton_template(&obj.template_name) {
                    burton_stealth_desired(true, obj.innate_stealth, obj.is_alive(), firing_primary)
                } else if is_jarmen_kell_template(&obj.template_name) {
                    jarmen_stealth_desired(true, obj.innate_stealth, obj.is_alive(), firing)
                } else if is_black_lotus_template(&obj.template_name) {
                    lotus_stealth_desired(
                        true,
                        obj.innate_stealth,
                        obj.is_alive(),
                        using_ability || firing,
                    )
                } else {
                    Some(!firing)
                };
                if let Some(want) = desired {
                    if want && !obj.status.stealthed {
                        if obj.stealth_allowed_frame > 0 && frame < obj.stealth_allowed_frame {
                            // C++ m_stealthAllowedFrame > now: wait StealthDelay.
                        } else {
                            obj.set_status_stealthed(true);
                            obj.set_status_detected(false);
                            obj.detection_expires_frame = 0;
                            obj.stealth_allowed_frame = 0;
                        }
                    } else if !want && obj.status.stealthed {
                        obj.break_stealth();
                        if obj.stealth_delay_pending {
                            obj.stealth_allowed_frame =
                                frame.saturating_add(obj.stealth_delay_frames);
                            obj.stealth_delay_pending = false;
                        }
                    }
                }
            }
        }

        // AmericaJetStealthFighter: InnateStealth Yes, Forbidden ATTACKING,
        // StealthDelay 2000ms (C++ StealthUpdate allowedToStealth).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                stealth_fighter_allowed_to_stealth_residual,
                STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL,
            };
            use crate::game_logic::host_stealth_fighter::is_stealth_fighter_template;
            let sf_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_alive() && is_stealth_fighter_template(&o.template_name))
                .map(|(id, _)| *id)
                .collect();
            for sid in sf_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                if !obj.innate_stealth {
                    obj.innate_stealth = true;
                    obj.record_host_stealth_flags();
                }
                if obj.stealth_delay_frames == 0 {
                    obj.stealth_delay_frames = STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL;
                }
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame =
                        frame.saturating_add(obj.stealth_delay_frames);
                    obj.stealth_delay_pending = false;
                } else if !obj.status.stealthed && obj.stealth_allowed_frame == 0 {
                    // C++ ctor: m_stealthAllowedFrame = now + StealthDelay.
                    obj.stealth_allowed_frame =
                        frame.saturating_add(obj.stealth_delay_frames);
                }
                let attacking = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                if stealth_fighter_allowed_to_stealth_residual(attacking) {
                    if obj.stealth_allowed_frame > 0 && frame < obj.stealth_allowed_frame {
                        // wait StealthDelay
                    } else if !obj.status.stealthed {
                        obj.set_status_stealthed(true);
                        obj.set_status_detected(false);
                        obj.detection_expires_frame = 0;
                        obj.stealth_allowed_frame = 0;
                    }
                } else if obj.status.stealthed {
                    obj.break_stealth();
                    if obj.stealth_delay_pending {
                        obj.stealth_allowed_frame =
                            frame.saturating_add(obj.stealth_delay_frames);
                        obj.stealth_delay_pending = false;
                    }
                }
            }
        }

        // Mines / demo traps / charges: InnateStealth + 0 opacity
        // (C++ StealthUpdate.cpp mine setEffectiveOpacity(0,0)).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                is_mine_stealth_kind_residual, MINE_STEALTH_OPACITY_RESIDUAL,
            };
            let mine_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.is_alive()
                        && is_mine_stealth_kind_residual(
                            o.is_kind_of(KindOf::Mine),
                            o.is_kind_of(KindOf::DemoTrap),
                            o.mine_data.is_some(),
                        )
                })
                .map(|(id, _)| *id)
                .collect();
            for mid in mine_ids {
                let Some(obj) = self.objects.get_mut(&mid) else {
                    continue;
                };
                if !obj.status.stealthed {
                    obj.apply_mine_innate_stealth();
                } else if (obj.camo_friendly_opacity - MINE_STEALTH_OPACITY_RESIDUAL).abs() > 1e-4 {
                    obj.camo_friendly_opacity = MINE_STEALTH_OPACITY_RESIDUAL;
                    obj.record_host_vision_camo();
                }
            }
        }

        // Collect active detectors (alive, not under construction) that are due
        // for a StealthDetectorUpdate DetectionRate residual scan.
        // Track residual detector kind for honesty counters.
        use crate::game_logic::host_strategy_center::{
            stealth_detector_hold_frames, stealth_detector_next_scan_frame,
            stealth_detector_scan_due,
        };
        #[derive(Clone, Copy)]
        struct DetFlags {
            is_sentry: bool,
            is_pathfinder: bool,
            is_scout: bool,
            is_listening_outpost: bool,
            is_troop_crawler: bool,
        }
        let mut detectors: Vec<(ObjectId, Team, Vec3, f32, DetFlags, u32)> = Vec::new();
        let mut scanned_detector_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in &self.objects {
            if !(o.is_detector
                && o.is_alive()
                && !o.status.under_construction
                && !o.status.destroyed)
            {
                continue;
            }
            let range = o.effective_detection_range();
            if range <= 0.0 {
                continue;
            }
            if !stealth_detector_scan_due(
                o.detection_rate_frames,
                o.next_detection_scan_frame,
                frame,
            ) {
                continue;
            }
            // C++ StealthDetectorUpdate.cpp:139-166 — contained detectors
            // sleep unless CanDetectWhileGarrisoned / CanDetectWhileTransported.
            if o.is_contained() {
                use crate::game_logic::host_radar_stealth_vision_residual::{
                    detector_can_scan_while_contained_residual,
                    DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL,
                    DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL,
                };
                let garrisonable = o
                    .contained_by
                    .and_then(|cid| self.objects.get(&cid))
                    .map(|c| {
                        c.thing.template.garrison_contain_max.is_some()
                            || (c.is_kind_of(KindOf::Structure) && c.building_data.is_some())
                    })
                    .unwrap_or(false);
                if !detector_can_scan_while_contained_residual(
                    true,
                    garrisonable,
                    DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL,
                    DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL,
                ) {
                    scanned_detector_ids.push(*id);
                    continue;
                }
            }
            let flags = DetFlags {
                is_sentry: crate::game_logic::host_sentry_drone::is_sentry_drone_template(
                    &o.template_name,
                ),
                is_pathfinder: crate::game_logic::host_pathfinder::is_pathfinder_template(
                    &o.template_name,
                ),
                is_scout: crate::game_logic::host_slave_drones::is_scout_drone_template(
                    &o.template_name,
                ),
                is_listening_outpost:
                    crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                        &o.template_name,
                    ) || o.is_listening_outpost_style_container(),
                is_troop_crawler: crate::game_logic::host_troop_crawler::is_troop_crawler_template(
                    &o.template_name,
                ) || o.is_troop_crawler_style_container(),
            };
            detectors.push((
                *id,
                o.team,
                o.get_position(),
                range,
                flags,
                o.detection_rate_frames,
            ));
            scanned_detector_ids.push(*id);
        }

        // Advance DetectionRate residual sleep for every detector that scanned
        // this tick (C++ returns UPDATE_SLEEP(m_updateRate) after each wake).
        for det_id in &scanned_detector_ids {
            if let Some(obj) = self.objects.get_mut(det_id) {
                if obj.detection_rate_frames > 0 {
                    obj.next_detection_scan_frame =
                        stealth_detector_next_scan_frame(obj.detection_rate_frames, frame);
                    self.stealth_detector_rate_scans =
                        self.stealth_detector_rate_scans.saturating_add(1);
                }
            }
        }

        if detectors.is_empty() {
            return;
        }

        // Collect stealthed targets first to avoid borrow conflicts.
        let stealthed_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && (o.status.stealthed || o.status.disguised))
            .map(|(id, _)| *id)
            .collect();

        for sid in stealthed_ids {
            let Some((s_team, s_pos, already_detected)) = self
                .objects
                .get(&sid)
                .map(|o| (o.team, o.get_position(), o.status.detected))
            else {
                continue;
            };

            let mut detected_by_sentry = false;
            let mut detected_by_pathfinder = false;
            let mut detected_by_scout = false;
            let mut detected_by_listening_outpost = false;
            let mut detected_by_troop_crawler = false;
            // C++ markAsDetected(updateRate + 1); take max hold among detecting scanners.
            let mut best_expires: u32 = 0;
            let mut spotting_detectors: Vec<ObjectId> = Vec::new();
            let detected_by_someone =
                detectors
                    .iter()
                    .any(|(id, det_team, det_pos, range, flags, rate)| {
                        let in_range = *det_team != s_team && det_pos.distance(s_pos) <= *range;
                        if in_range {
                            spotting_detectors.push(*id);
                            let hold = stealth_detector_hold_frames(*rate);
                            let exp = frame.saturating_add(hold);
                            if exp > best_expires {
                                best_expires = exp;
                            }
                            if flags.is_sentry {
                                detected_by_sentry = true;
                            }
                            if flags.is_pathfinder {
                                detected_by_pathfinder = true;
                            }
                            if flags.is_scout {
                                detected_by_scout = true;
                            }
                            if flags.is_listening_outpost {
                                detected_by_listening_outpost = true;
                            }
                            if flags.is_troop_crawler {
                                detected_by_troop_crawler = true;
                            }
                        }
                        in_range
                    });

            if detected_by_someone {
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.mark_detected(best_expires);
                }
                // Honesty: first residual reveal by residual detector kinds this tick.
                if !already_detected {
                    if detected_by_sentry {
                        self.sentry_drone_residual_detects =
                            self.sentry_drone_residual_detects.saturating_add(1);
                    }
                    if detected_by_pathfinder {
                        self.pathfinder_residual_detects =
                            self.pathfinder_residual_detects.saturating_add(1);
                    }
                    if detected_by_scout {
                        self.scout_drone_residual_detects =
                            self.scout_drone_residual_detects.saturating_add(1);
                    }
                    if detected_by_listening_outpost {
                        self.listening_outpost.record_detect();
                    }
                    if detected_by_troop_crawler {
                        self.troop_crawler.record_detect();
                    }
                    // C++ hero stealth detection EVA residual (Own/Enemy *Detected).
                    self.try_eva_hero_detected(sid);
                    // C++ StealthDetectorUpdate.cpp:199-260 radar/audio/message.
                    self.fire_stealth_discover_feedback(sid, &spotting_detectors);
                }
            }
        }
    }

    /// C++ StealthDetectorUpdate.cpp:199-260 first-detect radar/audio/UI.
    fn fire_stealth_discover_feedback(&mut self, target_id: ObjectId, detector_ids: &[ObjectId]) {
        use crate::game_logic::host_radar_stealth_vision_residual::{
            SPOTTER_AUDIO_STEALTH_DISCOVERED, SPOTTER_AUDIO_STEALTH_NEUTRALIZED,
            SPOTTER_MESSAGE_STEALTH_DISCOVERED, SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
        };
        use game_engine::common::system::radar::{
            get_radar_system, Coord3D, RadarEventType,
        };

        let Some(target) = self.objects.get(&target_id) else {
            return;
        };
        let t_team = target.team;
        let pos = target.get_position();
        let is_mine = target.is_kind_of(KindOf::Mine)
            || target.is_kind_of(KindOf::DemoTrap)
            || target.mine_data.is_some()
            || target.booby_trap_special;
        let loc = Coord3D {
            x: pos.x,
            y: pos.z,
            z: pos.y,
        };

        let local_team = self
            .players
            .values()
            .find(|p| p.is_local && p.is_alive)
            .map(|p| p.team);

        let local_detector = detector_ids.iter().any(|id| {
            self.objects.get(id).is_some_and(|d| {
                let local_ok = local_team.map(|lt| d.team == lt).unwrap_or(true);
                local_ok && d.team != t_team
            })
        });
        let local_victim = local_team.map(|lt| lt == t_team).unwrap_or(false);
        let enemy_detector = detector_ids.iter().any(|id| {
            self.objects.get(id).is_some_and(|d| d.team != t_team)
        });

        if local_detector {
            let do_feedback = if let Ok(mut radar) = get_radar_system().write() {
                radar.try_event(RadarEventType::StealthDiscovered, &loc)
            } else {
                true
            };
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_DISCOVERED,
                    "Stealth discovered",
                );
                self.queue_radar_message_at(
                    msg,
                    pos,
                    radar_notifications::RadarKind::Generic,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_DISCOVERED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
            }
        }

        if local_victim && enemy_detector {
            let do_feedback = if let Ok(mut radar) = get_radar_system().write() {
                if is_mine {
                    radar.try_event(RadarEventType::StealthNeutralized, &loc)
                } else {
                    radar.create_event(
                        &loc,
                        RadarEventType::StealthNeutralized,
                        crate::game_logic::host_radar_stealth_vision_residual::RADAR_EVENT_DEFAULT_SECONDS_TO_LIVE_RESIDUAL,
                    );
                    true
                }
            } else {
                true
            };
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
                    "Stealth neutralized",
                );
                self.queue_radar_message_at(
                    msg,
                    pos,
                    radar_notifications::RadarKind::Attack,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_NEUTRALIZED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
            }
        }
    }


    /// Place a residual land mine at `position` for `team`.
    pub fn place_land_mine(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::LandMine,
            "TestLandMine",
            team,
            position,
            producer,
            None,
            None,
        )
    }

    /// Place a residual GLA demo trap (proximity mode, standard detonation).
    pub fn place_demo_trap(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_demo_trap_named("TestDemoTrap", team, position, producer, false)
    }

    /// Place a residual demo trap with Chem/Demo/Standard profile from template name.
    ///
    /// `has_gamma` applies Chem Gamma death weapon residual when true.
    pub fn place_demo_trap_named(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        has_gamma: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{demo_trap_profile, HostMineData, HostMineKind};

        let profile = demo_trap_profile(template_name, has_gamma, false);
        self.ensure_residual_mine_template(template_name, HostMineKind::DemoTrap);
        let id = self.create_object(template_name, team, position)?;
        let mut data = HostMineData::demo_trap_with_profile(profile);
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            obj.record_host_demo_mine_cheer();
            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
            obj.thing.template.add_kind_of(KindOf::DemoTrap);
            obj.apply_mine_innate_stealth();
        }
        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(HostMineKind::DemoTrap.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        Some(id)
    }

    /// Place a residual timed demo charge (detonates after delay frames).
    pub fn place_timed_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            retail_timed_charge_lifetime_frames, retail_timed_charge_template,
        };
        let producer_template =
            producer.and_then(|pid| self.objects.get(&pid).map(|o| o.template_name.clone()));
        let template_name = retail_timed_charge_template(producer_template.as_deref());
        let delay =
            delay_frames.or_else(|| Some(retail_timed_charge_lifetime_frames(template_name)));
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge,
            template_name,
            team,
            position,
            producer,
            attach_to,
            delay,
        )
    }

    /// Place a residual remote demo charge (no auto-timer; remote detonate only).
    /// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
    pub fn place_remote_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::BURTON_REMOTE_CHARGE_OBJECT;
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge,
            BURTON_REMOTE_CHARGE_OBJECT,
            team,
            position,
            producer,
            attach_to,
            None,
        )
    }

    /// Detonate all residual remote demo charges planted by any of `producers`.
    /// Matches C++ SPECIAL_REMOTE_CHARGES no-target path (StickyBombUpdate::detonate).
    /// Returns the number of charges detonated.
    pub fn detonate_remote_demo_charges(&mut self, producers: &[ObjectId]) -> u32 {
        use crate::game_logic::host_mines::{HostMineDetonateReason, HostMineKind};

        if producers.is_empty() {
            return 0;
        }
        let producer_set: std::collections::HashSet<ObjectId> = producers.iter().copied().collect();
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if !data.is_active() || !obj.is_alive() {
                    return None;
                }
                if data.kind != HostMineKind::RemoteDemoCharge {
                    return None;
                }
                let producer = data.producer_id?;
                if !producer_set.contains(&producer) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut count = 0u32;
        for mine_id in due {
            if self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual) {
                count = count.saturating_add(1);
            }
        }
        if count > 0 {
            self.hero_abilities.record_remote_charge_detonate(count);
        }
        count
    }

    /// Cluster Mines special-power residual: place a ring of land mines.
    /// Fail-closed: not full OCL ClusterMinesBomb / GenerateMinefieldBehavior density.
    pub fn place_cluster_mines(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            cluster_mine_positions, CLUSTER_MINE_COUNT, CLUSTER_MINE_RING_RADIUS,
        };
        let positions =
            cluster_mine_positions(center, CLUSTER_MINE_COUNT, CLUSTER_MINE_RING_RADIUS);
        let mut ids = Vec::with_capacity(positions.len());
        for pos in positions {
            if let Some(id) = self.place_land_mine(team, pos, producer) {
                ids.push(id);
            }
        }
        if !ids.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new("MineFieldPlaced")
                    .with_position(center)
                    .with_priority(160),
            );
        }
        ids
    }

    pub(in super::super) fn ensure_residual_mine_template(
        &mut self,
        template_name: &str,
        kind: crate::game_logic::host_mines::HostMineKind,
    ) {
        if self.templates.contains_key(template_name) {
            return;
        }
        let mut t = ThingTemplate::new(template_name);
        // Mines are not infantry/vehicles; residual treats them as Neutral objects
        // with mine_data driving behavior. Demo trap is structure-like but residual
        // does not require full structure production path.
        match kind {
            crate::game_logic::host_mines::HostMineKind::DemoTrap => {
                t.add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Selectable)
                    .add_kind_of(KindOf::DemoTrap)
                    .set_health(100.0)
                    .set_cost(400, 0);
            }
            crate::game_logic::host_mines::HostMineKind::LandMine
            | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
            | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                t.add_kind_of(KindOf::Mine).set_health(100.0).set_cost(0, 0);
            }
        }
        self.templates.insert(template_name.to_string(), t);
    }

    pub(in super::super) fn place_mine_kind(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            can_place_remote_charge, can_place_timed_charge, HostMineData, HostMineKind,
            BURTON_UNIQUE_CHARGE_TARGETS,
        };

        // C++ MaxSpecialObjects + UniqueSpecialObjectTargets residual (Burton C4).
        if matches!(
            kind,
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
        ) {
            if let Some(pid) = producer {
                let mut timed_n = 0u32;
                let mut remote_n = 0u32;
                for o in self.objects.values() {
                    if !o.is_alive() {
                        continue;
                    }
                    let Some(md) = o.mine_data.as_ref() else {
                        continue;
                    };
                    if md.detonated {
                        continue;
                    }
                    let owned = md.producer_id == Some(pid) || o.producer_id == Some(pid);
                    if !owned {
                        continue;
                    }
                    match md.kind {
                        HostMineKind::TimedDemoCharge => timed_n = timed_n.saturating_add(1),
                        HostMineKind::RemoteDemoCharge => remote_n = remote_n.saturating_add(1),
                        _ => {}
                    }
                }
                match kind {
                    HostMineKind::TimedDemoCharge if !can_place_timed_charge(timed_n) => {
                        return None;
                    }
                    HostMineKind::RemoteDemoCharge if !can_place_remote_charge(remote_n) => {
                        return None;
                    }
                    _ => {}
                }
            }
            if BURTON_UNIQUE_CHARGE_TARGETS {
                if let Some(tid) = attach_to {
                    let dup = self.objects.values().any(|o| {
                        o.is_alive()
                            && o.mine_data
                                .as_ref()
                                .map(|m| {
                                    !m.detonated
                                        && matches!(
                                            m.kind,
                                            HostMineKind::TimedDemoCharge
                                                | HostMineKind::RemoteDemoCharge
                                        )
                                        && m.attached_to == Some(tid)
                                })
                                .unwrap_or(false)
                    });
                    if dup {
                        return None;
                    }
                }
            }
        }

        self.ensure_residual_mine_template(template_name, kind);
        let id = self.create_object(template_name, team, position)?;

        let mut data = match kind {
            crate::game_logic::host_mines::HostMineKind::LandMine => HostMineData::land_mine(),
            crate::game_logic::host_mines::HostMineKind::DemoTrap => HostMineData::demo_trap(),
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge => {
                let mut d = HostMineData::timed_demo_charge(self.frame);
                if let Some(delay) = delay_frames {
                    d = d.with_lifetime_frames(self.frame, delay);
                }
                d
            }
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                HostMineData::remote_demo_charge()
            }
        };
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(t) = attach_to {
            data = data.with_attach(t);
        }

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            obj.record_host_demo_mine_cheer();
            // Mines/charges are not combat movers.
            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
            match kind {
                crate::game_logic::host_mines::HostMineKind::DemoTrap => {
                    obj.thing.template.add_kind_of(KindOf::DemoTrap);
                }
                _ => {
                    obj.thing.template.add_kind_of(KindOf::Mine);
                }
            }
            obj.apply_mine_innate_stealth();
        }

        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(kind.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        Some(id)
    }

    /// Manually detonate a residual demo trap / charge (command residual).
    pub fn manual_detonate_mine(&mut self, mine_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::HostMineDetonateReason;
        self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual)
    }

    /// Advance residual mines: dozer clear + proximity scan + timed detonation.
    ///
    /// Clear residual (C++ DozerMineDisarmingWeapon DAMAGE_DISARM): only when
    /// WEAPONSET_MINE_CLEARING_DETAIL is armed (MSG_SET_MINE_CLEARING_DETAIL).
    /// Demo-trap skip is DISARM-while-attacking only (DemoTrapUpdate.cpp:181-191).
    /// Proximity fuse is ENEMIES only (DemoTrapUpdate.cpp:196).

    /// C++ StickyBombUpdate::update residual.
    ///
    /// Timed/remote demo charges with `attached_to` follow the target position
    /// (vehicle: ride on roof offset Z; structure/immobile: stay at ground height).
    /// If the target dies, the charge is destroyed (C++ destroyObject(self)).
    pub(crate) fn update_sticky_bomb_attachments(&mut self) {
        use crate::game_logic::host_mines::HostMineKind;
        /// Retail StickyBombUpdate OffsetZ residual (ride on vehicle roof).
        const STICKY_OFFSET_Z: f32 = 8.0;

        let sticky_ids: Vec<(ObjectId, ObjectId, HostMineKind)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if !md.is_active() || !obj.is_alive() {
                    return None;
                }
                if !matches!(
                    md.kind,
                    HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
                ) {
                    return None;
                }
                let tid = md.attached_to?;
                Some((*id, tid, md.kind))
            })
            .collect();

        let mut destroy_charges: Vec<ObjectId> = Vec::new();
        let mut moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();

        for (charge_id, target_id, _kind) in sticky_ids {
            let Some(target) = self.objects.get(&target_id) else {
                destroy_charges.push(charge_id);
                continue;
            };
            if !target.is_alive() || target.status.effectively_dead {
                destroy_charges.push(charge_id);
                continue;
            }
            let tpos = target.get_position();
            let immobile =
                target.is_kind_of(KindOf::Structure) || target.is_kind_of(KindOf::Immobile);
            let new_pos = if immobile {
                // Keep ground height for mine-clearing units (C++ IMMOBILE path).
                glam::Vec3::new(tpos.x, 0.0, tpos.z)
            } else {
                glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Z, tpos.z)
            };
            // If structure path kept original plant XY, we still snap to target XY
            // residual for moving vehicles; immobile also snaps XY to structure center
            // for host residual simplicity (fail-closed vs bomber plant XY freeze).
            moves.push((charge_id, new_pos));
        }

        for (charge_id, pos) in moves {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                obj.set_position(pos);
            }
            self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
        }
        for charge_id in destroy_charges {
            self.sticky_bomb_target_deaths = self.sticky_bomb_target_deaths.saturating_add(1);
            self.destroy_object(charge_id);
        }
    }

    /// C++ RemoteC4Charge SpecialObjectsPersistWhenOwnerDies = No residual.
    /// TimedC4Charge persists (BURTON_TIMED_PERSIST_WHEN_OWNER_DIES = true).
    pub fn cleanup_remote_charges_when_owner_dies(&mut self) {
        use crate::game_logic::host_mines::HostMineKind;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if md.detonated || !obj.is_alive() {
                    return None;
                }
                if md.kind != HostMineKind::RemoteDemoCharge {
                    return None;
                }
                let pid = md.producer_id.or(obj.producer_id)?;
                let owner_dead = self
                    .objects
                    .get(&pid)
                    .map(|p| !p.is_alive() || p.status.destroyed || p.status.effectively_dead)
                    .unwrap_or(true);
                if owner_dead {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                if let Some(md) = o.mine_data.as_mut() {
                    md.detonated = true;
                }
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn update_mines_and_demo_traps(&mut self) {
        use crate::game_logic::host_mines::{
            can_clear_mine_kind, demo_trap_skips_dozer_disarm_while_attacking, is_mine_clearer,
            mine_clear_allowed_for_order, HostMineDetonateReason, HostMineKind,
            DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE,
        };

        let frame = self.frame;
        // C++ StickyBombUpdate::update residual — stick to target / die with target.
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_sticky_bomb_attachments();
        }
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_booby_trap_special_attachments();
        }
        // C++ SpecialObjectsPersistWhenOwnerDies = No for RemoteC4Charge residual.
        self.cleanup_remote_charges_when_owner_dies();
        let mut due: Vec<(ObjectId, HostMineDetonateReason)> = Vec::new();
        let mut clear_due: Vec<(ObjectId, ObjectId)> = Vec::new(); // (mine_id, clearer_id)
        let mut approach: Vec<(ObjectId, Vec3)> = Vec::new(); // clearer moves toward mine
        // C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
        for (id, obj) in &self.objects {
            let Some(data) = obj.mine_data.as_ref() else {
                continue;
            };
            if crate::game_logic::host_mines::should_detonate_when_killed(
                data.kind,
                crate::game_logic::host_mines::DEMO_TRAP_DETONATE_WHEN_KILLED,
                !obj.is_alive(),
                data.detonated,
                obj.status.under_construction || obj.status.sold,
            ) {
                due.push((*id, HostMineDetonateReason::Killed));
            }
        }

        // Collect active mine positions + params first (avoid borrow issues).
        let mines: Vec<(
            ObjectId,
            Team,
            Vec3,
            f32,
            bool,
            Option<u32>,
            bool,
            HostMineKind,
        )> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if !data.is_active() || !obj.is_alive() {
                    return None;
                }
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    data.trigger_range,
                    data.proximity_enabled,
                    data.detonate_at_frame,
                    obj.status.under_construction,
                    data.kind,
                ))
            })
            .collect();

        // Mine clearers: only after player/AI issued MINE_CLEARING_DETAIL.
        let clearers: Vec<(ObjectId, Team, Vec3, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                if !is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name) {
                    return None;
                }
                if !mine_clear_allowed_for_order(obj.weapon_set_mine_clearing_detail) {
                    return None;
                }
                let busy = matches!(
                    obj.ai_state,
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Capturing
                        | AIState::SpecialAbility
                );
                Some((*id, obj.team, obj.get_position(), busy))
            })
            .collect();

        // Potential victims: dozers skip only DISARM + IS_ATTACKING (C++ :181-191).
        let victims: Vec<(ObjectId, Team, Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                let is_dozer =
                    is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name);
                let weapon_disarm = obj
                    .weapon_name_for_slot(0)
                    .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                    .unwrap_or(false)
                    || obj.weapon_set_mine_clearing_detail;
                let attacking = obj.status.attacking
                    || matches!(obj.ai_state, AIState::Attacking);
                if demo_trap_skips_dozer_disarm_while_attacking(is_dozer, weapon_disarm, attacking)
                {
                    return None;
                }
                let combatant = obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Attackable);
                if !combatant {
                    return None;
                }
                if obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target {
                    return None;
                }
                Some((*id, obj.team, obj.get_position()))
            })
            .collect();

        // Dozer/Worker clear + approach residual before proximity (so clear wins).
        // C++: only enemy/neutral mines (not ally/own) — residual uses team inequality.
        let clear_range_sqr = DOZER_MINE_CLEAR_RANGE * DOZER_MINE_CLEAR_RANGE;
        let scan_range_sqr = DOZER_MINE_CLEAR_SCAN_RANGE * DOZER_MINE_CLEAR_SCAN_RANGE;
        for (cid, cteam, cpos, busy) in &clearers {
            if *busy {
                continue;
            }
            // Pure residual acquire: nearest clearable enemy/neutral mine in scan range (XZ).
            let mine_cands: Vec<_> = mines
                .iter()
                .filter(|(_, mine_team, _, _, _, _, under_construction, kind)| {
                    !*under_construction && can_clear_mine_kind(*kind) && *mine_team != *cteam
                })
                .map(|(mine_id, mine_team, mine_pos, _, _, _, _, _)| {
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: *mine_id,
                        team: *mine_team,
                        position: *mine_pos,
                        is_alive: true,
                        is_neutral: *mine_team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    }
                })
                .collect();
            let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(*cid),
                (cpos.x, cpos.z),
                mine_cands,
                DOZER_MINE_CLEAR_SCAN_RANGE,
                |_| true,
            )
            .map(|(mine_id, dist, _)| {
                let mine_pos = mines
                    .iter()
                    .find(|(id, _, _, _, _, _, _, _)| *id == mine_id)
                    .map(|(_, _, p, _, _, _, _, _)| *p)
                    .unwrap_or(*cpos);
                (mine_id, dist * dist, mine_pos)
            });
            // Keep scan_range_sqr referenced for residual parity with prior sqr gate.
            let _ = scan_range_sqr;
            if let Some((mine_id, dist_sqr, mine_pos)) = best {
                if dist_sqr <= clear_range_sqr {
                    // Prefer first clearer to claim a mine this frame.
                    if !clear_due.iter().any(|(m, _)| *m == mine_id) {
                        clear_due.push((mine_id, *cid));
                    }
                } else {
                    // Approach residual: move idle clearer toward nearest mine.
                    approach.push((*cid, mine_pos));
                }
            }
        }

        for (
            mine_id,
            mine_team,
            mine_pos,
            trigger_range,
            proximity,
            detonate_at,
            under_construction,
            _,
        ) in &mines
        {
            if *under_construction {
                continue;
            }
            // Already scheduled for safe clear this frame — do not also detonate.
            if clear_due.iter().any(|(m, _)| *m == *mine_id) {
                continue;
            }
            if let Some(at) = detonate_at {
                if frame >= *at {
                    due.push((*mine_id, HostMineDetonateReason::Timed));
                    continue;
                }
            }
            if !proximity || *trigger_range <= 0.0 {
                continue;
            }
            let range_sqr = trigger_range * trigger_range;
            for (vid, _vteam, vpos) in &victims {
                if *vid == *mine_id {
                    continue;
                }
                // C++ DemoTrapUpdate.cpp:195-204: `getObject()->getRelationship(other)
                // != ENEMIES` skips (unless FriendlyDetonation). Same-faction enemies
                // are ENEMIES via Player::getRelationship, not Team enum equality.
                if self.mine_proximity_skips_friendly(*mine_id, *vid) {
                    continue;
                }
                let dx = vpos.x - mine_pos.x;
                let dz = vpos.z - mine_pos.z;
                if dx * dx + dz * dz <= range_sqr {
                    due.push((*mine_id, HostMineDetonateReason::Proximity));
                    break;
                }
            }
        }

        // Safe clears first (mine gone, no splash).
        for (mine_id, clearer_id) in clear_due {
            let _ = self.clear_mine_internal(mine_id, clearer_id);
        }

        // Idle clearer approach: set move toward nearest enemy mine.
        for (clearer_id, mine_pos) in approach {
            if let Some(obj) = self.objects.get_mut(&clearer_id) {
                // Don't clobber an explicit non-idle order already in flight.
                if matches!(
                    obj.ai_state,
                    AIState::Idle | AIState::Moving | AIState::Attacking
                ) || obj.target.is_none()
                {
                    obj.set_ai_state(AIState::Moving);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(clearer_id, 1);
                    }
                    obj.movement.target_position = Some(mine_pos);
                    crate::game_logic::host_move_log::record(
                        clearer_id,
                        Some([mine_pos.x, mine_pos.y, mine_pos.z]),
                    );
                    obj.set_status_moving(true);
                }
            }
        }

        for (mine_id, reason) in due {
            let _ = self.detonate_mine_internal(mine_id, reason);
        }
    }

    /// Safely disarm/clear a residual mine without detonation or area damage.
    /// C++ Weapon DAMAGE_DISARM → LandMineInterface::disarm / destroyObject residual.
    pub fn clear_mine_internal(&mut self, mine_id: ObjectId, clearer_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::{can_clear_mine_kind, MINE_CLEARED_AUDIO};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if !mine.is_alive() {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated || !can_clear_mine_kind(data.kind) {
            return false;
        }
        let clearer_team = self.objects.get(&clearer_id).map(|o| o.team);
        if clearer_team == Some(mine.team) {
            // Never clear own/ally residual mines.
            return false;
        }
        let mine_pos = mine.get_position();

        // Mark disarmed (detonated flag reuses "no longer active" residual bookkeeping).
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
                md.proximity_enabled = false;
                md.detonate_at_frame = None;
            }
        }

        self.mine_residual_clears = self.mine_residual_clears.saturating_add(1);

        self.queue_audio_event(
            AudioEventRequest::new(MINE_CLEARED_AUDIO)
                .with_object(mine_id)
                .with_position(mine_pos)
                .with_priority(160),
        );

        // Destroy mine without splash damage (DAMAGE_DISARM residual).
        self.mark_object_for_destruction(mine_id, None);

        // Clearer stays alive — no damage applied.
        if let Some(clearer) = self.objects.get_mut(&clearer_id) {
            if clearer.target == Some(mine_id) {
                clearer.target = None;
            }
            if matches!(clearer.ai_state, AIState::Attacking | AIState::Moving) {
                clearer.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(clearer_id, 0);
                }
                clearer.movement.target_position = None;
                clearer.set_status_moving(false);
                clearer.set_status_attacking(false);
            }
        }

        true
    }

    pub(in super::super) fn detonate_mine_internal(
        &mut self,
        mine_id: ObjectId,
        reason: crate::game_logic::host_mines::HostMineDetonateReason,
    ) -> bool {
        use crate::game_logic::host_mines::{damage_at_distance, HostMineDetonateReason};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if !mine.is_alive() && !matches!(reason, HostMineDetonateReason::Killed) {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated {
            return false;
        }
        if mine.status.under_construction {
            return false;
        }

        let kind = data.kind;
        let damage = data.detonation_damage;
        let radius = data.detonation_radius;
        let demo_profile = data.demo_trap_profile;
        let is_demo_trap = matches!(kind, crate::game_logic::host_mines::HostMineKind::DemoTrap);
        let mine_team = mine.team;
        let mine_pos = mine.get_position();
        let producer = data.producer_id;

        // Mark detonated before applying damage.
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
            }
        }

        match reason {
            HostMineDetonateReason::Proximity => {
                self.mine_residual_proximity_detonations =
                    self.mine_residual_proximity_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Timed => {
                self.mine_residual_timed_detonations =
                    self.mine_residual_timed_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Manual | HostMineDetonateReason::Killed => {
                self.mine_residual_manual_detonations =
                    self.mine_residual_manual_detonations.saturating_add(1);
            }
        }

        // Area damage: residual hits enemies + neutrals; demo trap / sticky charges
        // also hit allies (DemoTrap/TNT RadiusDamageAffects SELF ALLIES ENEMIES NEUTRALS).
        let hit_allies = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::DemoTrap
                | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );

        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == mine_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() || victim.mine_data.is_some() {
                continue;
            }
            if victim.team == mine_team && !hit_allies {
                continue;
            }
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - mine_pos.x;
                let dz = vpos.z - mine_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            let dmg = if is_demo_trap {
                crate::game_logic::host_mines::demo_trap_damage_at(demo_profile, dist)
            } else {
                damage_at_distance(damage, radius, dist)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                if victim.take_damage_from(dmg, Some(mine_id)) {
                    destroy_ids.push((vid, mine_team));
                }
            }
        }

        // Chem DemoTrap residual: spawn MediumPoisonField at detonation.
        if is_demo_trap && demo_profile.spawns_poison() {
            let _ = self.toxin_tractor.spawn_medium_field(
                mine_id,
                mine_team,
                mine_pos,
                self.frame,
                demo_profile.poison_anthrax_tier(),
            );
        }

        // Audio + particle residual.
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(mine_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            mine_pos,
            self.frame,
            Some(mine_id),
            None,
        );

        // Destroy the mine/trap itself.
        self.mark_object_for_destruction(mine_id, producer.map(|_| mine_team));
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }

        let _ = producer; // residual bookkeeping only
        true
    }

    /// C++ `Object::getRelationship` / `DemoTrapUpdate.cpp:196`.
    /// Detonate only on ENEMIES. Neutral and Allies never start the fuse.
    fn mine_proximity_skips_friendly(&self, mine_id: ObjectId, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::demo_trap_proximity_requires_enemies;
        use gamelogic::common::Relationship;

        let Some(mine) = self.objects.get(&mine_id) else {
            return true;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return true;
        };
        let rel = match (
            self.player_owner_for_host_object(mine),
            self.player_owner_for_host_object(victim),
        ) {
            (Some(mine_owner), Some(victim_owner)) => {
                self.player_relationship(mine_owner, victim_owner)
            }
            _ => {
                if victim.team == mine.team {
                    Relationship::Allies
                } else if victim.team == Team::Neutral || mine.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
        };
        !demo_trap_proximity_requires_enemies(rel == Relationship::Enemies)
    }
}
