//! Stealth and detector updates.

use super::*;

impl GameLogic {
    /// Host residual for C++ StealthUpdate + StealthDetectorUpdate targetability.
    ///
    /// - Expires `OBJECT_STATUS_DETECTED` when `detection_expires_frame` is reached
    /// - Detectors mark nearby enemy stealthed units as detected (hold ~1s)
    /// - Bomb truck disguise reveal residual (RevealDistanceFromTarget = 100)
    /// - Sentry / Camo Rebel / heroes apply StealthDelay before re-cloak
    /// - Stealth Fighter + mines innate cloak; contained detectors do not scan
    pub fn update_stealth_and_detection(&mut self) {
        self.update_stealth_and_detection_with_effects(&mut Default::default())
    }

    pub(crate) fn update_stealth_and_detection_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let frame = self.frame;
        // C++ StealthUpdate.cpp:725-780 cloak/destalth/detect SoundStealthOn/Off.
        let stealth_snap: Vec<(ObjectId, bool, bool)> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, o)| (*id, o.status.stealthed, o.status.detected))
            .collect();

        // Expire timed detections (unit may remain stealthed).
        // C++ StealthUpdate.cpp:768-801 — DETECTED clear on a garrison rider
        // refreshes the container's apparent controller / hide flag.
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for obj in self.objects.registered_values_mut() {
            if obj.status.detected
                && obj.detection_expires_frame > 0
                && frame >= obj.detection_expires_frame
            {
                obj.clear_detected();
                if let Some(cid) = obj.contained_by {
                    garrison_detect_recalc.push(cid);
                }
            }
        }
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);

        // C++ StealthUpdate.cpp:365-373 — occupants of non-garrison transports destalth.
        // Contained-and-forbidden re-arms StealthDelay every frame (cpp:737-739).
        {
            let destalth: Vec<ObjectId> = self
                .objects
                .registered()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    let container = self.objects.get(&obj.contained_by?)?;
                    let garrisonable = container.is_garrison_contain()
                        || container.thing().template.garrison_contain_max.is_some();
                    if Object::transport_contain_should_destalth(garrisonable) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for id in destalth {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_stealth_allowed_update(frame, false);
                }
            }
        }

        // Bomb truck disguise: RevealDistanceFromTarget only when getCurrentVictim
        // exists and FROM_CENTER_2D <= 100. Attack-move / ground fire have no victim.
        {
            use crate::game_logic::host_bomb_truck_disguise::should_reveal_disguise_by_distance;
            let disguised: Vec<(ObjectId, Option<ObjectId>, Vec3)> = self
                .objects
                .registered()
                .filter(|(_, o)| o.status.disguised && o.is_alive())
                .map(|(id, o)| (*id, o.target, o.get_position()))
                .collect();
            let mut reveal_ids: Vec<ObjectId> = Vec::new();
            for (id, victim_id, pos) in disguised {
                let Some(vid) = victim_id else {
                    continue;
                };
                let Some(victim) = self.objects.get(&vid) else {
                    continue;
                };
                let dist = Object::stealth_detector_distance_2d(pos, victim.get_position());
                if should_reveal_disguise_by_distance(dist) {
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
            }
        }

        // Pathfinder residual: leftover destalth only when leftover velocity
        // exceeds leftover MoveThresholdSpeed (C++ StealthUpdate.cpp:389-392).
        {
            use crate::game_logic::host_pathfinder::{
                PATHFINDER_MOVE_THRESHOLD_SPEED, pathfinder_stealth_desired,
            };
            use gamelogic::stealth_update::leftover_stealth_move_threshold_speed;
            // Class bit set at spawn — no per-frame template-name scan.
            let pf_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| o.is_pathfinder_unit && o.is_alive())
                .map(|(id, _)| *id)
                .collect();
            for pid in pf_ids {
                let Some(obj) = self.objects.get_mut(&pid) else {
                    continue;
                };
                if !obj.stealth_or_detector_update_processes() {
                    continue;
                }
                if obj.script_unstealthed {
                    if obj.status.stealthed {
                        obj.break_stealth();
                    }
                    continue;
                }
                let leftover_velocity = obj.velocity_magnitude();
                let leftover_speed = leftover_stealth_move_threshold_speed(&obj.template_name)
                    .unwrap_or(PATHFINDER_MOVE_THRESHOLD_SPEED);
                if let Some(desired) = pathfinder_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    leftover_velocity,
                    leftover_speed,
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
                is_sentry_drone_template, sentry_stealth_desired,
            };
            let sentry_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| o.is_alive() && is_sentry_drone_template(&o.template_name))
                .map(|(id, _)| *id)
                .collect();
            for sid in sentry_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                let leftover_velocity = obj.velocity_magnitude();
                let leftover_speed =
                    gamelogic::stealth_update::leftover_stealth_move_threshold_speed(
                        &obj.template_name,
                    )
                    .unwrap_or(0.0);
                let moving = gamelogic::stealth_update::leftover_not_while_moving_destalths(
                    leftover_velocity,
                    leftover_speed,
                );
                let firing = obj.stealth_fired_primary_recently(frame);
                // Delay is applied in apply_stealth_allowed_update, not allowedToStealth.
                let Some(allowed) = sentry_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.is_alive(),
                    moving,
                    firing,
                    frame,
                    0,
                ) else {
                    continue;
                };
                obj.apply_stealth_allowed_update(frame, allowed);
            }
        }

        // Listening Outpost residual: StealthForbiddenConditions = MOVING RIDERS_ATTACKING.
        // C++ StealthUpdate.cpp:737-739 re-arms StealthDelay (2000ms / 60f) every forbidden frame.
        {
            use crate::game_logic::host_listening_outpost::{
                LISTENING_OUTPOST_STEALTH_DELAY_FRAMES, listening_outpost_stealth_desired,
            };
            // Style bit installed at spawn for LO templates — no name scan.
            let lo_ids: Vec<ObjectId> = self
                .objects
                .registered()
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
                                    rider.ai_state(),
                                    AIState::Attacking
                                        | AIState::AttackMoving
                                        | AIState::AttackingGround
                                ))
                    })
                });
                let Some(obj) = self.objects.get_mut(&lid) else {
                    continue;
                };
                if obj.stealth_delay_frames == 0 {
                    obj.stealth_delay_frames = LISTENING_OUTPOST_STEALTH_DELAY_FRAMES;
                }
                if !obj.status.stealthed
                    && obj.stealth_allowed_frame == 0
                    && !obj.stealth_delay_pending
                {
                    // C++ ctor: m_stealthAllowedFrame = now + StealthDelay.
                    obj.rearm_stealth_delay(frame);
                }
                let leftover_velocity = obj.velocity_magnitude();
                let leftover_speed =
                    gamelogic::stealth_update::leftover_stealth_move_threshold_speed(
                        &obj.template_name,
                    )
                    .unwrap_or(0.0);
                let moving = gamelogic::stealth_update::leftover_not_while_moving_destalths(
                    leftover_velocity,
                    leftover_speed,
                );
                if let Some(allowed) = listening_outpost_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    moving,
                    riders_attacking,
                ) {
                    obj.apply_stealth_allowed_update(frame, allowed);
                }
            }
        }

        // GLA Camouflage residual: StealthForbiddenConditions = ATTACKING USING_ABILITY,
        // StealthDelay 2500ms (75f) before re-cloak. C++ GLAInfantryRebel StealthUpdate.
        {
            use crate::game_logic::host_upgrades::{
                UPGRADE_GLA_CAMOUFLAGE, camouflage_unit_stealth_desired,
            };

            // Upgrade tag is only applied to camouflage-eligible units at unlock.
            let camo_ids: Vec<ObjectId> = self
                .objects
                .registered()
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
                let attacking = obj.stealth_is_firing_weapon();
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state(), AIState::SpecialAbility);
                // Delay is applied in apply_stealth_allowed_update, not allowedToStealth.
                let Some(allowed) = camouflage_unit_stealth_desired(
                    obj.innate_stealth,
                    obj.is_alive(),
                    attacking,
                    using_ability,
                    frame,
                    0,
                ) else {
                    continue;
                };
                obj.apply_stealth_allowed_update(frame, allowed);
            }
        }

        // GLA CamoNetting structure residual: StealthForbiddenConditions =
        // ATTACKING + USING_ABILITY + TAKING_DAMAGE + NO_BLACK_MARKET,
        // StealthDelay 2500ms re-cloak,
        // OrderIdleEnemiesToAttackMeUponReveal residual on uncloak,
        // FriendlyOpacity residual (min cloaked / max revealed + pulse while cloaked),
        // StealthLook / heat-vision second-pass residual (Drawable::setStealthLook).
        {
            use crate::game_logic::host_black_market::is_black_market_template;
            use crate::game_logic::host_upgrades::{
                CAMO_NETTING_FRIENDLY_OPACITY_MAX, CAMO_NETTING_FRIENDLY_OPACITY_MIN,
                UPGRADE_GLA_CAMO_NETTING, camo_netting_heat_vision_opacity,
                camo_netting_order_idle_enemy_in_range, camo_netting_pulse_opacity,
                camo_netting_stealth_look, camo_netting_structure_stealth_desired,
                is_camo_netting_structure_template, stealth_same_controlling_player,
            };

            let live_markets: Vec<(Team, Option<u32>)> = self
                .objects
                .registered_values()
                .filter(|o| {
                    crate::game_logic::object::is_live_stealth_black_market(
                        o.is_kind_of(KindOf::FSBlackMarket)
                            || is_black_market_template(&o.template_name),
                        o.is_kind_of(KindOf::FSFake),
                        o.is_alive(),
                        o.status.under_construction,
                        o.status.sold,
                        o.status.destroyed,
                    )
                })
                .map(|o| (o.team, o.owner_player_id))
                .collect();
            let struct_ids: Vec<ObjectId> = self
                .objects
                .registered()
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
                let attacking = obj.stealth_is_firing_weapon();
                // C++ OBJECT_STATUS_IS_USING_ABILITY residual.
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state(), AIState::SpecialAbility);
                let taking_damage = obj.stealth_taking_non_healing_damage(frame);
                let has_live_black_market = live_markets.iter().any(|(team, player)| {
                    stealth_same_controlling_player(obj.team, obj.owner_player_id, *team, *player)
                });
                // Delay is applied in apply_stealth_allowed_update, not allowedToStealth.
                let Some(allowed) = camo_netting_structure_stealth_desired(
                    obj.innate_stealth,
                    obj.is_alive(),
                    attacking,
                    using_ability,
                    frame,
                    0,
                    has_live_black_market,
                ) else {
                    continue;
                };
                let allowed = allowed && !taking_damage;
                let was_stealthed = obj.status.stealthed;
                obj.apply_stealth_allowed_update(frame, allowed);
                if obj.status.stealthed && !was_stealthed {
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MIN;

                    obj.camo_opacity_pulse_phase = 0.0;
                    opacity_cloaks = opacity_cloaks.saturating_add(1);
                    recloaks = recloaks.saturating_add(1);
                } else if !obj.status.stealthed && was_stealthed {
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;

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

                    obj.camo_opacity_pulse_phase = next_phase;
                } else {
                    // Revealed residual: hold max opacity.
                    if (obj.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() > 0.01
                    {
                        obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;

                        opacity_reveals = opacity_reveals.saturating_add(1);
                    }
                }

                // StealthLook residual for enemy observer (default host residual view).
                // Detected stealthed structures → heat-vision second material pass.
                // C++ setStealthLook writes opacity only when look *changes*;
                // Drawable::draw fades the residual between detector pulses.
                let look = camo_netting_stealth_look(
                    obj.status.stealthed,
                    obj.status.detected,
                    false, // enemy observer residual (non-friendly)
                );
                let hv = camo_netting_heat_vision_opacity(look);
                let look_u8 = look.as_u8();
                let look_changed = obj.camo_stealth_look != look_u8;
                if look_changed && hv > 0.0 {
                    heat_vision = heat_vision.saturating_add(1);
                }
                if look_changed {
                    let disguised = obj.status.disguised
                        || obj.disguise_as_template.is_some()
                        || obj.disguise_pending_template.is_some();
                    obj.queue_changed_stealth_material(look_u8, disguised);
                }
                obj.camo_stealth_look = look_u8;

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
                    .registered()
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
                            if sr > 0.0 { sr } else { 150.0 }
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
                    let idle = matches!(enemy.ai_state(), AIState::Idle) && enemy.target.is_none();
                    if !camo_netting_order_idle_enemy_in_range(
                        enemy.is_alive(),
                        idle,
                        can_attack,
                        dist,
                        vision,
                    ) {
                        continue;
                    }
                    enemy.set_target_with_operation(
                        Some(rid),
                        &mut self.attack_events,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut effects.reborrow_for_object(&mut self.partition_manager),
                    );
                    {}
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
            use crate::game_logic::host_hero_abilities::lotus_stealth_desired;
            use crate::game_logic::host_jarmen_kell::{
                is_jarmen_kell_template, jarmen_stealth_desired,
            };
            use crate::game_logic::host_radar_stealth_vision_residual::hero_stealth_delay_frames_residual;
            let hero_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && o.contained_by.is_none()
                        && (is_colonel_burton_template(&o.template_name)
                            || is_jarmen_kell_template(&o.template_name)
                            || o.thing().template.capture_power
                                == crate::game_logic::CapturePowerKind::BlackLotus
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
                let firing = obj.stealth_is_firing_weapon();
                // C++ FIRING_PRIMARY only — Burton knife (secondary) does not destalth.
                let firing_primary = obj.stealth_fired_primary_recently(frame);
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state(), AIState::SpecialAbility);
                let allowed = if is_colonel_burton_template(&obj.template_name) {
                    burton_stealth_desired(true, obj.innate_stealth, obj.is_alive(), firing_primary)
                } else if is_jarmen_kell_template(&obj.template_name) {
                    jarmen_stealth_desired(true, obj.innate_stealth, obj.is_alive(), firing)
                } else if obj.thing().template.capture_power
                    == crate::game_logic::CapturePowerKind::BlackLotus
                {
                    lotus_stealth_desired(
                        true,
                        obj.innate_stealth,
                        obj.is_alive(),
                        using_ability || firing,
                    )
                } else {
                    Some(!firing)
                };
                if let Some(allowed) = allowed {
                    obj.apply_stealth_allowed_update(frame, allowed);
                }
            }
        }

        // Combat Cycle UseRiderStealth: first rider owns delay / CAN_STEALTH.
        // C++ StealthUpdate.cpp:531-556 calcStealthOwner; effects stay on the bike.
        {
            use crate::game_logic::host_combat_cycle::{
                COMBAT_CYCLE_USE_RIDER_STEALTH, CombatCycleRider,
                combat_cycle_rider_stealth_desired, combat_cycle_stealth_owner_rider,
                is_combat_cycle_template, rider_from_template_name, rider_grants_can_stealth,
                rider_stealth_delay_frames,
            };
            let bike_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| o.is_alive() && o.is_combat_cycle_style_container())
                .map(|(id, _)| *id)
                .collect();
            for bid in bike_ids {
                let Some(bike) = self.objects.get(&bid) else {
                    continue;
                };
                let residual = CombatCycleRider::from_u8(bike.combat_cycle_rider);
                let occupant = bike.contained_units().first().copied();
                let (first_rider, occupant_can_stealth, occupant_delay) =
                    if let Some(oid) = occupant {
                        self.objects
                            .get(&oid)
                            .map(|occ| {
                                let rider = rider_from_template_name(&occ.template_name);
                                let can = occ.innate_stealth || rider_grants_can_stealth(rider);
                                let delay = if occ.stealth_delay_frames > 0 {
                                    occ.stealth_delay_frames
                                } else {
                                    rider_stealth_delay_frames(rider)
                                };
                                (Some(rider), can, delay)
                            })
                            .unwrap_or((
                                None,
                                rider_grants_can_stealth(residual),
                                rider_stealth_delay_frames(residual),
                            ))
                    } else {
                        (
                            None,
                            rider_grants_can_stealth(residual),
                            rider_stealth_delay_frames(residual),
                        )
                    };
                let rider = combat_cycle_stealth_owner_rider(
                    COMBAT_CYCLE_USE_RIDER_STEALTH,
                    residual,
                    first_rider,
                );
                let rider_can = if first_rider.is_some() {
                    occupant_can_stealth
                } else {
                    rider_grants_can_stealth(rider)
                };
                let delay = if first_rider.is_some() && occupant_delay > 0 {
                    occupant_delay
                } else {
                    rider_stealth_delay_frames(rider)
                };

                let Some(obj) = self.objects.get_mut(&bid) else {
                    continue;
                };
                obj.stealth_delay_frames = delay;
                let attacking = obj.stealth_is_firing_weapon();
                let Some(want) = combat_cycle_rider_stealth_desired(
                    true,
                    COMBAT_CYCLE_USE_RIDER_STEALTH,
                    rider,
                    rider_can,
                    obj.is_alive(),
                    attacking,
                ) else {
                    continue;
                };
                obj.apply_stealth_allowed_update(frame, want);
            }
        }

        // Aircraft with authored StealthUpdate InnateStealth Yes + Forbidden
        // ATTACKING (retail AmericaJetStealthFighter, StealthDelay 2000ms).
        // Keyed on the template's StealthUpdate module data, as C++
        // StealthUpdate::allowedToStealth is — never the template name.
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL, residual_ms_to_frames,
                stealth_fighter_allowed_to_stealth_residual,
            };
            // Keyed on the authored weapon, not the template name (hq-2qedt.225).
            use crate::game_logic::host_stealth_fighter::STEALTH_JET_MISSILE_WEAPON;
            let sf_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| {
                    o.is_alive() && o.weapon_name_for_slot(0) == Some(STEALTH_JET_MISSILE_WEAPON)
                })
                .map(|(id, _)| *id)
                .collect();
            for sid in sf_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                if !obj.innate_stealth {
                    obj.innate_stealth = true;
                }
                if obj.stealth_delay_frames == 0 {
                    obj.stealth_delay_frames = STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL;
                }
                if !obj.status.stealthed
                    && obj.stealth_allowed_frame == 0
                    && !obj.stealth_delay_pending
                {
                    // C++ ctor: m_stealthAllowedFrame = now + StealthDelay.
                    obj.stealth_allowed_frame = frame.saturating_add(obj.stealth_delay_frames);
                }
                let attacking = obj.stealth_is_firing_weapon();
                obj.apply_stealth_allowed_update(
                    frame,
                    stealth_fighter_allowed_to_stealth_residual(attacking),
                );
            }
        }

        // Mines / demo traps / charges: InnateStealth + 0 opacity
        // (C++ StealthUpdate.cpp mine setEffectiveOpacity(0,0)).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                MINE_STEALTH_OPACITY_RESIDUAL, is_mine_stealth_kind_residual,
            };
            let mine_ids: Vec<ObjectId> = self
                .objects
                .registered()
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
                } else {
                    obj.apply_mine_no_attack_from_ai();
                    if (obj.camo_friendly_opacity - MINE_STEALTH_OPACITY_RESIDUAL).abs() > 1e-4 {
                        obj.camo_friendly_opacity = MINE_STEALTH_OPACITY_RESIDUAL;
                    }
                }
            }
        }

        // GPS / receiveGrant residual: CAN_STEALTH units destalthed by fire
        // re-cloak after StealthDelay only when allowedToStealth (StealthUpdate.cpp:717-735).
        {
            use crate::game_logic::host_black_market::is_black_market_template;
            use crate::game_logic::host_upgrades::is_camo_netting_structure_template;
            let live_markets: Vec<(Team, Option<u32>)> = self
                .objects
                .registered_values()
                .filter(|o| {
                    crate::game_logic::object::is_live_stealth_black_market(
                        o.is_kind_of(KindOf::FSBlackMarket)
                            || is_black_market_template(&o.template_name),
                        o.is_kind_of(KindOf::FSFake),
                        o.is_alive(),
                        o.status.under_construction,
                        o.status.sold,
                        o.status.destroyed,
                    )
                })
                .map(|o| (o.team, o.owner_player_id))
                .collect();
            let grant_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && !o.status.stealthed
                        && o.contained_by.is_none()
                        && !Object::temporary_stealth_grant_should_expire(
                            o.temporary_stealth_expires_frame,
                            frame,
                            matches!(
                                o.ai_state(),
                                AIState::Moving
                                    | AIState::Attacking
                                    | AIState::AttackMoving
                                    | AIState::AttackingGround
                            ),
                        )
                })
                .map(|(id, _)| *id)
                .collect();
            for gid in grant_ids {
                let occupants = self
                    .objects
                    .get(&gid)
                    .map(|o| o.contained_units())
                    .unwrap_or_default();
                let riders_attacking = occupants.iter().any(|rid| {
                    self.objects.get(rid).is_some_and(|rider| {
                        rider.is_alive()
                            && (rider.status.attacking
                                || matches!(
                                    rider.ai_state(),
                                    AIState::Attacking
                                        | AIState::AttackMoving
                                        | AIState::AttackingGround
                                ))
                    })
                });
                let (requires_black_market, has_live_black_market) = self
                    .objects
                    .get(&gid)
                    .map(|obj| {
                        let requires = is_camo_netting_structure_template(&obj.template_name)
                            && obj.stealth_breaks_on_damage;
                        let has = live_markets.iter().any(|(team, player)| {
                            crate::game_logic::host_upgrades::stealth_same_controlling_player(
                                obj.team,
                                obj.owner_player_id,
                                *team,
                                *player,
                            )
                        });
                        (requires, has)
                    })
                    .unwrap_or((false, false));
                let Some(obj) = self.objects.get_mut(&gid) else {
                    continue;
                };
                let leftover_velocity = obj.velocity_magnitude();
                let leftover_speed =
                    gamelogic::stealth_update::leftover_stealth_move_threshold_speed(
                        &obj.template_name,
                    )
                    .unwrap_or(0.0);
                let moving = gamelogic::stealth_update::leftover_not_while_moving_destalths(
                    leftover_velocity,
                    leftover_speed,
                );
                let forbidden = obj.stealth_level_forbids_cloak(
                    frame,
                    moving,
                    riders_attacking,
                    requires_black_market,
                    has_live_black_market,
                );
                obj.try_recloak_after_stealth_delay(frame, forbidden);
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
            extra_required: u128,
            extra_forbidden: u128,
        }
        let mut detectors: Vec<(ObjectId, Team, Option<u32>, Vec3, f32, DetFlags, u32)> =
            Vec::new();
        let mut scanned_detector_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.registered() {
            if !(o.is_detector
                && o.is_alive()
                && !o.status.under_construction
                && !o.status.destroyed
                && !o.status.sold)
            {
                continue;
            }
            if !o.stealth_or_detector_update_processes() {
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
                    DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL,
                    DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL,
                    detector_can_scan_while_contained_residual,
                };
                let garrisonable = o
                    .contained_by
                    .and_then(|cid| self.objects.get(&cid))
                    .map(|c| {
                        c.thing().template.garrison_contain_max.is_some()
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
            let (extra_required, extra_forbidden) =
                (o.extra_detect_kindof, o.extra_detect_kindof_not);
            let flags = DetFlags {
                is_sentry: crate::game_logic::host_sentry_drone::is_sentry_drone_template(
                    &o.template_name,
                ),
                is_pathfinder: o.authors_pathfinder_sniper_weapon(),
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
                extra_required,
                extra_forbidden,
            };
            detectors.push((
                *id,
                o.team,
                o.owner_player_id,
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
            self.apply_local_stealth_look_and_heat_vision();
            self.play_stealth_transition_sounds(&stealth_snap);
            return;
        }

        // Leftover StealthDetectorUpdate::update clears IR grids at each scan
        // wake, then re-creates them for every in-range detected target.
        {
            let scanning: Vec<ObjectId> = detectors.iter().map(|d| d.0).collect();
            self.clear_detector_ir_grids_for(&scanning, effects);
        }

        // Collect stealthed targets first to avoid borrow conflicts.
        let stealthed_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive() && (o.status.stealthed || o.status.disguised))
            .map(|(id, _)| *id)
            .collect();
        let mut spotted_detectors: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();

        for sid in stealthed_ids {
            let Some((s_team, s_owner, s_pos, already_detected, s_kind)) =
                self.objects.get(&sid).map(|o| {
                    (
                        o.team,
                        o.owner_player_id,
                        o.get_position(),
                        o.status.detected,
                        o.kind_of_cpp_mask(),
                    )
                })
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
                    .any(|(id, det_team, det_owner, det_pos, range, flags, rate)| {
                        // C++ PartitionFilterRelationship ALLOW_ENEMIES|ALLOW_NEUTRAL.
                        let rel_ok = match (*det_owner, s_owner) {
                            (Some(a), Some(b)) => {
                                use gamelogic::common::Relationship;
                                matches!(
                                    self.player_relationship(a, b),
                                    Relationship::Enemies | Relationship::Neutral
                                )
                            }
                            _ => *det_team != s_team,
                        };
                        // C++ PartitionFilterAcceptByKindOf ExtraRequired/ForbiddenKindOf.
                        let kind_ok = crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                            s_kind,
                            flags.extra_required,
                            flags.extra_forbidden,
                        );
                        // C++ StealthDetectorUpdate.cpp:179-180 FROM_CENTER_2D
                        // (host Y-up → XZ; altitude must not shrink radius).
                        let in_range = rel_ok
                            && kind_ok
                            && Object::stealth_detector_distance_2d(*det_pos, s_pos) <= *range;
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
                for id in &spotting_detectors {
                    spotted_detectors.insert(*id);
                }
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.mark_detected(best_expires);
                    obj.apply_detected_heat_vision_second_pass();
                }
                // C++ StealthUpdate::markAsDetected orderIdlesToAttack walk.
                self.order_idle_enemies_to_attack_on_reveal(sid);
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
                    // C++ StealthDetectorUpdate.cpp:199-260 radar/audio/message.
                    // Hero Enemy*/Own* EVA is inside doFeedback (tryEvent 10s).
                    self.fire_stealth_discover_feedback(sid, &spotting_detectors);
                }
                // C++ StealthDetectorUpdate.cpp:292-308 / leftover: IR grid is
                // outside the !was_detected feedback gate — every scan refresh.
                for det_id in &spotting_detectors {
                    let Some(det_y) = detectors.iter().find(|d| d.0 == *det_id).map(|d| d.3.y)
                    else {
                        continue;
                    };
                    self.spawn_detector_ir_grid_at(s_pos, det_y, *det_id, effects);
                }
            }
        }

        // C++ StealthUpdate.cpp:786-801 — occupant DETECTED flip refreshes
        // GarrisonContain hide / capture so enemies see GARRISONED.
        let garrison_detect_recalc: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(_, o)| {
                if o.status.detected {
                    o.contained_by
                } else {
                    None
                }
            })
            .collect();
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);

        // C++ PartitionFilterStealthedOrStealthGarrisoned + occupant
        // markAsDetected(updateRate+2) (StealthDetectorUpdate.cpp:105-117, :311-333).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::spotter_mark_detected_garrison_rider_frames_residual;
            let buildings: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| o.is_alive() && o.is_garrison_contain())
                .map(|(id, _)| *id)
                .collect();
            let mut touched: Vec<ObjectId> = Vec::new();
            for bid in buildings {
                let Some((b_pos, occupants, b_kind)) = self
                    .objects
                    .get(&bid)
                    .map(|b| (b.get_position(), b.contained_units(), b.kind_of_cpp_mask()))
                else {
                    continue;
                };
                let stealth_riders: Vec<ObjectId> = occupants
                    .into_iter()
                    .filter(|id| {
                        self.objects
                            .get(id)
                            .is_some_and(|o| o.is_alive() && o.status.stealthed)
                    })
                    .collect();
                if stealth_riders.is_empty() {
                    continue;
                }
                for (_id, det_team, det_owner, det_pos, range, flags, rate) in &detectors {
                    if Object::stealth_detector_distance_2d(*det_pos, b_pos) > *range {
                        continue;
                    }
                    // C++ applies PartitionFilterAcceptByKindOf to the container `them`.
                    if !crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        b_kind,
                        flags.extra_required,
                        flags.extra_forbidden,
                    ) {
                        continue;
                    }

                    let hold = spotter_mark_detected_garrison_rider_frames_residual(*rate);
                    let expires = frame.saturating_add(hold);
                    let mut marked = false;
                    for rid in &stealth_riders {
                        let Some((r_team, r_owner)) =
                            self.objects.get(rid).map(|o| (o.team, o.owner_player_id))
                        else {
                            continue;
                        };
                        let rel_ok = match (*det_owner, r_owner) {
                            (Some(a), Some(b)) => {
                                use gamelogic::common::Relationship;
                                matches!(
                                    self.player_relationship(a, b),
                                    Relationship::Enemies | Relationship::Neutral
                                )
                            }
                            _ => *det_team != r_team,
                        };
                        if !rel_ok {
                            continue;
                        }
                        if let Some(obj) = self.objects.get_mut(rid) {
                            obj.mark_detected(expires);
                            marked = true;
                        }
                        self.order_idle_enemies_to_attack_on_reveal(*rid);
                    }
                    if marked {
                        touched.push(bid);
                    }
                }
            }
            self.recalc_garrisons_after_occupant_detect_change(&touched);
        }
        let scanned: Vec<ObjectId> = detectors.iter().map(|d| d.0).collect();
        self.play_detector_ir_scan_fx(&scanned, &spotted_detectors, effects);
        self.apply_local_stealth_look_and_heat_vision();
        self.play_stealth_transition_sounds(&stealth_snap);
    }

    /// C++ StealthUpdate.cpp:725-780 SoundStealthOn / SoundStealthOff.
    pub(super) fn play_stealth_transition_sounds(&mut self, before: &[(ObjectId, bool, bool)]) {
        use crate::game_logic::object::{SOUND_STEALTH_OFF, SOUND_STEALTH_ON};
        let local = self.local_player_id();
        let mut events: Vec<(ObjectId, Vec3, &'static str)> = Vec::new();
        for &(id, was_stealthed, was_detected) in before {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            let pos = obj.get_position();
            let local_ok =
                crate::game_logic::source_is_locally_controlled(obj.owner_player_id, local);
            if obj.status.stealthed != was_stealthed {
                // Cloak and destalth both play StealthOn in retail.
                events.push((id, pos, SOUND_STEALTH_ON));
            }
            if obj.status.detected && !was_detected {
                events.push((id, pos, SOUND_STEALTH_OFF));
            } else if !obj.status.detected && was_detected && local_ok {
                events.push((id, pos, SOUND_STEALTH_ON));
            }
        }
        for (id, pos, name) in events {
            self.queue_audio_event(
                AudioEventRequest::new(name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(140),
            );
        }
    }

    /// C++ StealthDetectorUpdate.cpp:338-394 IR ping / beacon after DetectionRate.
    pub(super) fn play_detector_ir_scan_fx(
        &mut self,
        detector_ids: &[ObjectId],
        spotted: &std::collections::HashSet<ObjectId>,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_radar_stealth_vision_residual::{
            DETECTOR_IR_BEACON_PARTICLE, DETECTOR_IR_BRIGHT_PARTICLE, DETECTOR_IR_LOUD_PING_SOUND,
            DETECTOR_IR_PING_PARTICLE, DETECTOR_IR_PING_SOUND,
        };
        let frame = self.frame;
        let mut events: Vec<(ObjectId, Vec3, bool)> = Vec::new();
        for id in detector_ids {
            if !self.detector_ir_fx_visible_to_local(*id) {
                continue;
            }
            let Some(pos) = self.objects.get(id).map(|o| o.get_position()) else {
                continue;
            };
            events.push((*id, pos, spotted.contains(id)));
        }
        for (id, pos, found) in events {
            let sound = if found {
                DETECTOR_IR_LOUD_PING_SOUND
            } else {
                DETECTOR_IR_PING_SOUND
            };
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(150),
            );
            let ping = if found {
                DETECTOR_IR_BRIGHT_PARTICLE
            } else {
                DETECTOR_IR_PING_PARTICLE
            };
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::ParticleSysBone,
                ping,
                pos,
                frame,
                Some(id),
                None,
                particle_owner,
            );
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::ParticleSysBone,
                DETECTOR_IR_BEACON_PARTICLE,
                pos,
                frame,
                Some(id),
                None,
                particle_owner,
            );
        }
    }

    /// C++ StealthDetectorUpdate.cpp:341-343: shroud-clear and not stealthed-to-local.
    pub(super) fn detector_ir_fx_visible_to_local(&self, det_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&det_id) else {
            return false;
        };
        if obj.status.stealthed && !self.is_object_locally_controlled(det_id) {
            return false;
        }
        // C++ Object::getShroudedStatus (Object.cpp:1778-1788): ALWAYS_VISIBLE
        // or no PartitionData (garrisoned) → CLEAR.
        if obj.get_template().always_visible || obj.contained_by.is_some() {
            return true;
        }
        let Some(local) = self.local_player_id() else {
            return true;
        };
        let status = self
            .world_services
            .shroud()
            .lock()
            .ok()
            .and_then(|mgr| mgr.get_host_object_shroud_status(local, det_id.0));
        match status {
            Some(shroud) => {
                (shroud as u8) <= (gamelogic::common::types::ObjectShroudStatus::PartialClear as u8)
            }
            // C++ Object.cpp:1786-1788: no PartitionData → CLEAR.
            None => true,
        }
    }

    /// Leftover StealthDetectorUpdate::clear_grid_particles at each scan wake.
    pub(super) fn clear_detector_ir_grids_for(
        &mut self,
        detector_ids: &[ObjectId],
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::host_radar_stealth_vision_residual::DETECTOR_IR_GRID_PARTICLE;
        let stale: Vec<u32> = self
            .combat_particles
            .active_systems()
            .filter(|e| {
                e.template_name == DETECTOR_IR_GRID_PARTICLE
                    && e.source_object
                        .is_some_and(|src| detector_ids.contains(&src))
            })
            .map(|e| e.id)
            .collect();
        for id in stale {
            self.combat_particles.deactivate(id, particle_owner);
        }
    }

    /// C++ StealthDetectorUpdate.cpp:292-308 IR grid each DetectionRate scan.
    /// Host Y-up: snap XZ %12, height = detector Y + 17 (C++ detector z + 17).
    pub(super) fn spawn_detector_ir_grid_at(
        &mut self,
        pos: Vec3,
        detector_y: f32,
        detector_id: ObjectId,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_radar_stealth_vision_residual::DETECTOR_IR_GRID_PARTICLE;
        let grid = Vec3::new(
            pos.x - pos.x.rem_euclid(12.0),
            detector_y + 17.0,
            pos.z - pos.z.rem_euclid(12.0),
        );
        let _ = self.combat_particles.spawn_named(
            CombatParticleKind::ParticleSysBone,
            DETECTOR_IR_GRID_PARTICLE,
            grid,
            self.frame,
            Some(detector_id),
            None,
            particle_owner,
        );
    }

    /// C++ StealthUpdate.cpp:809-810 local-player StealthLook + heat-vision pass.
    /// CamoNetting structures keep their existing enemy-observer residual write.
    pub(super) fn apply_local_stealth_look_and_heat_vision(&mut self) {
        use crate::game_logic::host_upgrades::{
            calc_stealthed_status_for_player, is_camo_netting_structure_template,
        };
        let local = self.local_player_id();
        let local_inactive = local
            .and_then(|id| self.players.get(&id))
            .map(|p| !p.is_alive)
            .unwrap_or(true);
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if is_camo_netting_structure_template(&obj.template_name)
                && obj.stealth_breaks_on_damage
            {
                continue;
            }
            let friendly = local_inactive
                || self.is_object_locally_controlled(id)
                || match (obj.owner_player_id, local) {
                    (Some(a), Some(b)) => {
                        self.player_relationship(a, b) == gamelogic::common::Relationship::Allies
                    }
                    _ => false,
                };
            let local_owner = self.is_object_locally_controlled(id);
            let hint = !obj.status.stealthed
                && obj.innate_stealth
                && local_owner
                && (obj.stealth_is_firing_weapon()
                    || obj.status.using_ability
                    || matches!(obj.ai_state(), AIState::SpecialAbility));
            let can_disguise =
                crate::game_logic::host_bomb_truck_disguise::has_disguises_as_team_stealth_residual(
                    &obj.template_name,
                );
            // C++ isDisguised() is `m_disguiseAsTemplate != NULL` (set on
            // disguiseAsObject, before halfpoint). Host keeps the template in
            // pending until halfpoint, then `status.disguised`.
            let is_disguised = obj.status.disguised
                || obj.disguise_as_template.is_some()
                || obj.disguise_pending_template.is_some();
            let look = calc_stealthed_status_for_player(
                obj.status.stealthed,
                obj.status.detected,
                friendly,
                can_disguise,
                is_disguised,
            );
            if let Some(obj) = self.objects.get_mut(&id) {
                // Leftover StealthUpdate.cpp:666-670 pulse for every enabled
                // non-mine, non-disguise-transition module (not just Camo Netting).
                if !is_camo_netting_structure_template(&obj.template_name) {
                    obj.apply_stealth_update_pulse();
                }
                if hint {
                    // StealthUpdate.cpp:748 precedes changed setStealthLook:810.
                    obj.queue_second_material_pass(1.0);
                }
                let look_u8 = look.as_u8();
                let look_changed = obj.camo_stealth_look != look_u8;
                if look_changed {
                    obj.queue_changed_stealth_material(look_u8, is_disguised);
                    obj.camo_stealth_look = look_u8;
                    obj.update_drawable_hidden_status();
                }
            }
            self.admit_object_drawable_deselection(id);
        }
        self.drain_hidden_drawable_selection();
        self.drain_masked_object_selection();
    }

    /// C++ StealthDetectorUpdate.cpp:199-260 first-detect radar/audio/UI.
    pub(crate) fn fire_stealth_discover_feedback(
        &mut self,
        target_id: ObjectId,
        detector_ids: &[ObjectId],
    ) {
        use crate::game_logic::host_radar_stealth_vision_residual::{
            SPOTTER_AUDIO_STEALTH_DISCOVERED, SPOTTER_AUDIO_STEALTH_NEUTRALIZED,
            SPOTTER_MESSAGE_STEALTH_DISCOVERED, SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
        };
        use game_engine::common::system::radar::{Coord3D, RadarEventType};

        let Some(target) = self.objects.get(&target_id) else {
            return;
        };
        let t_team = target.team;
        let t_owner = target.owner_player_id;
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

        // C++ StealthDetectorUpdate.cpp:202-203 / :244-245:
        // local player == detector/victim controller, relationship != ALLIES.
        let local_detector = detector_ids.iter().any(|id| {
            if !self.is_object_locally_controlled(*id) {
                return false;
            }
            let Some(d) = self.objects.get(id) else {
                return false;
            };
            match (d.owner_player_id, t_owner) {
                (Some(a), Some(b)) => {
                    self.player_relationship(a, b) != gamelogic::common::Relationship::Allies
                }
                _ => d.team != t_team,
            }
        });
        let local_victim = self.is_object_locally_controlled(target_id);
        let enemy_detector = detector_ids.iter().any(|id| {
            let Some(d) = self.objects.get(id) else {
                return false;
            };
            match (d.owner_player_id, t_owner) {
                (Some(a), Some(b)) => {
                    self.player_relationship(a, b) != gamelogic::common::Relationship::Allies
                }
                _ => d.team != t_team,
            }
        });

        if local_detector {
            let do_feedback = self
                .radar_mut()
                .try_event(RadarEventType::StealthDiscovered, &loc);
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_DISCOVERED,
                    "Stealth discovered",
                );
                self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_DISCOVERED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
                // C++ :233-237 EnemyDetectionEvaEvent inside doFeedback.
                self.try_eva_hero_detected_kind(target_id, false);
            }
        }

        if local_victim && enemy_detector {
            let do_feedback = {
                let radar = self.radar_mut();
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
            };
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
                    "Stealth neutralized",
                );
                self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_NEUTRALIZED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
                // C++ :269-274 OwnDetectionEvaEvent inside doFeedback.
                self.try_eva_hero_detected_kind(target_id, true);
            }
        }
    }
}
