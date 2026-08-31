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
        // C++ Drawable::draw fades m_secondMaterialPassOpacity * 0.8 every frame
        // before the next detector pulse re-arms 1.0.
        for obj in self.objects.values_mut() {
            obj.fade_heat_vision_second_pass();
        }

        // C++ StealthUpdate.cpp:725-780 cloak/destalth/detect SoundStealthOn/Off.
        let stealth_snap: Vec<(ObjectId, bool, bool)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, o)| (*id, o.status.stealthed, o.status.detected))
            .collect();

        // Expire timed detections (unit may remain stealthed).
        // C++ StealthUpdate.cpp:768-801 — DETECTED clear on a garrison rider
        // refreshes the container's apparent controller / hide flag.
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for obj in self.objects.values_mut() {
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
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
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
                .iter()
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
                .iter()
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
                .iter()
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
                let attacking = obj.stealth_is_firing_weapon();
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
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
                .values()
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
                let attacking = obj.stealth_is_firing_weapon();
                // C++ OBJECT_STATUS_IS_USING_ABILITY residual.
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
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
                    obj.record_host_vision_camo();
                    obj.camo_opacity_pulse_phase = 0.0;
                    opacity_cloaks = opacity_cloaks.saturating_add(1);
                    recloaks = recloaks.saturating_add(1);
                } else if !obj.status.stealthed && was_stealthed {
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
                if look_changed && hv > 0.0 && obj.camo_heat_vision_opacity < 0.5 {
                    heat_vision = heat_vision.saturating_add(1);
                }
                if look_changed {
                    obj.camo_heat_vision_opacity = hv;
                } else if hv == 0.0 {
                    obj.camo_heat_vision_opacity = 0.0;
                }
                obj.camo_stealth_look = look_u8;
                obj.record_host_vision_camo();
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
                        && o.contained_by.is_none()
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
                let firing = obj.stealth_is_firing_weapon();
                // C++ FIRING_PRIMARY only — Burton knife (secondary) does not destalth.
                let firing_primary = obj.stealth_fired_primary_recently(frame);
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
                let allowed = if is_colonel_burton_template(&obj.template_name) {
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
                .iter()
                .filter(|(_, o)| {
                    o.is_alive()
                        && (is_combat_cycle_template(&o.template_name)
                            || o.is_combat_cycle_style_container())
                })
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

        // AmericaJetStealthFighter: InnateStealth Yes, Forbidden ATTACKING,
        // StealthDelay 2000ms (C++ StealthUpdate allowedToStealth).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL,
                stealth_fighter_allowed_to_stealth_residual,
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
                } else {
                    obj.apply_mine_no_attack_from_ai();
                    if (obj.camo_friendly_opacity - MINE_STEALTH_OPACITY_RESIDUAL).abs() > 1e-4 {
                        obj.camo_friendly_opacity = MINE_STEALTH_OPACITY_RESIDUAL;
                        obj.record_host_vision_camo();
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
                .values()
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
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && !o.status.stealthed
                        && o.contained_by.is_none()
                        && !Object::temporary_stealth_grant_should_expire(
                            o.temporary_stealth_expires_frame,
                            frame,
                            matches!(
                                o.ai_state,
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
                                    rider.ai_state,
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
        for (id, o) in &self.objects {
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
            let (extra_required, extra_forbidden) =
                crate::game_logic::host_radar_stealth_vision_residual::extra_detect_kindof_for_detector(
                    &o.template_name,
                    o.extra_detect_kindof,
                    o.extra_detect_kindof_not,
                );
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
            self.clear_detector_ir_grids_for(&scanning);
        }

        // Collect stealthed targets first to avoid borrow conflicts.
        let stealthed_ids: Vec<ObjectId> = self
            .objects
            .iter()
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
                    self.spawn_detector_ir_grid_at(s_pos, det_y, *det_id);
                }
            }
        }

        // C++ StealthUpdate.cpp:786-801 — occupant DETECTED flip refreshes
        // GarrisonContain hide / capture so enemies see GARRISONED.
        let garrison_detect_recalc: Vec<ObjectId> = self
            .objects
            .iter()
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
                .iter()
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
        self.play_detector_ir_scan_fx(&scanned, &spotted_detectors);
        self.apply_local_stealth_look_and_heat_vision();
        self.play_stealth_transition_sounds(&stealth_snap);
    }

    /// C++ StealthUpdate.cpp:725-780 SoundStealthOn / SoundStealthOff.
    fn play_stealth_transition_sounds(&mut self, before: &[(ObjectId, bool, bool)]) {
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
    fn play_detector_ir_scan_fx(
        &mut self,
        detector_ids: &[ObjectId],
        spotted: &std::collections::HashSet<ObjectId>,
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
            );
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::ParticleSysBone,
                DETECTOR_IR_BEACON_PARTICLE,
                pos,
                frame,
                Some(id),
                None,
            );
        }
    }

    /// C++ StealthDetectorUpdate.cpp:341-343: shroud-clear and not stealthed-to-local.
    fn detector_ir_fx_visible_to_local(&self, det_id: ObjectId) -> bool {
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
        let status = gamelogic::system::shroud_manager::get_shroud_manager()
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
    fn clear_detector_ir_grids_for(&mut self, detector_ids: &[ObjectId]) {
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
            self.combat_particles.deactivate(id);
        }
    }

    /// C++ StealthDetectorUpdate.cpp:292-308 IR grid each DetectionRate scan.
    /// Host Y-up: snap XZ %12, height = detector Y + 17 (C++ detector z + 17).
    fn spawn_detector_ir_grid_at(&mut self, pos: Vec3, detector_y: f32, detector_id: ObjectId) {
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
        );
    }

    /// C++ StealthUpdate.cpp:809-810 local-player StealthLook + heat-vision pass.
    /// CamoNetting structures keep their existing enemy-observer residual write.
    fn apply_local_stealth_look_and_heat_vision(&mut self) {
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
            .iter()
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
            let is_mine = obj.is_kind_of(KindOf::Mine)
                || obj.is_kind_of(KindOf::DemoTrap)
                || obj.mine_data.is_some();
            let local_owner = self.is_object_locally_controlled(id);
            let hint = !obj.status.stealthed
                && obj.innate_stealth
                && local_owner
                && (obj.stealth_is_firing_weapon()
                    || obj.status.using_ability
                    || matches!(obj.ai_state, AIState::SpecialAbility));
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
            let hv = crate::game_logic::stealth_second_material_pass_opacity(
                obj.status.stealthed,
                obj.status.detected,
                obj.status.disguised
                    || crate::game_logic::host_bomb_truck_disguise::has_disguises_as_team_stealth_residual(
                        &obj.template_name,
                    ),
                is_mine,
                !obj.is_alive(),
                hint,
            );
            if let Some(obj) = self.objects.get_mut(&id) {
                // Leftover StealthUpdate.cpp:666-670 pulse for every enabled
                // non-mine, non-disguise-transition module (not just Camo Netting).
                if !is_camo_netting_structure_template(&obj.template_name) {
                    obj.apply_stealth_update_pulse();
                }
                let look_u8 = look.as_u8();
                let look_changed = obj.camo_stealth_look != look_u8;
                obj.camo_stealth_look = look_u8;
                if look_changed {
                    // C++ setStealthLook writes m_secondMaterialPassOpacity only on change.
                    obj.camo_heat_vision_opacity = hv;
                    // C++ setStealthLook(STEALTHLOOK_INVISIBLE) → updateHiddenStatus.
                    obj.update_drawable_hidden_status();
                } else if hint {
                    // C++ hintDetectableWhileUnstealthed re-arms 1.0 each destalth update.
                    obj.camo_heat_vision_opacity = hv;
                } else if !obj.status.stealthed && !hint {
                    obj.camo_heat_vision_opacity = hv;
                } else if obj.status.stealthed && !obj.status.detected && !is_mine {
                    obj.camo_heat_vision_opacity = 0.0;
                }
                obj.record_host_vision_camo();
            }
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
        use game_engine::common::system::radar::{Coord3D, RadarEventType, get_radar_system};

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
        use crate::game_logic::host_mines::{HostMineData, HostMineKind, demo_trap_profile};

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
            if self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual, None) {
                count = count.saturating_add(1);
            }
        }
        if count > 0 {
            self.hero_abilities.record_remote_charge_detonate(count);
        }
        count
    }

    /// Cluster Mines special-power residual: SmartBorder field around the drop.
    /// C++ ClusterMinesBomb GenerateMinefieldBehavior SmartBorder + DistanceAroundObject 80.
    pub fn place_cluster_mines(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            apply_cluster_mines_drop_variance, cluster_mines_drop_unit_samples,
        };
        let seed = producer.map(|p| p.0).unwrap_or(0).wrapping_add(self.frame);
        let (ux, uy) = cluster_mines_drop_unit_samples(seed);
        let center = apply_cluster_mines_drop_variance(center, ux, uy);
        self.place_cluster_mines_unvaried(team, center, producer)
    }

    /// SmartBorder around an already-offset drop (cargo-bomb impact).
    pub fn place_cluster_mines_unvaried(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            CLUSTER_MINES_MINE_TEMPLATE, cluster_smart_border_positions,
        };
        // Leftover GenerateMinefieldBehavior::place_mines play_fx GenerationFX
        // at the bomb/object center (`TheFXList::do_fx_at_position`).
        let _ =
            crate::game_logic::host_cluster_mines_flight::play_cluster_mines_generation_fx(center);

        let positions = cluster_smart_border_positions(center);
        let mut ids = Vec::with_capacity(positions.len());
        for pos in positions {
            if self.mine_spot_blocked(pos) {
                continue;
            }
            if let Some(id) =
                self.place_land_mine_named(CLUSTER_MINES_MINE_TEMPLATE, team, pos, producer)
            {
                // C++ ClusterMinesBomb is the producer; scoot from impact, not the plane.
                self.apply_land_mine_scoot(id, center, pos);
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

    /// C++ GenerateMinefieldBehavior::placeMineAt underwater / cliff / under-structure skip.
    fn mine_spot_blocked(&self, pos: Vec3) -> bool {
        use crate::game_logic::host_mines::{LAND_MINE_GEOMETRY_RADIUS, mine_spot_under_structure};
        if let Some(terrain) = self.terrain.as_ref() {
            if terrain.is_underwater_at_world(pos) || terrain.is_cliff_at_world(pos) {
                return true;
            }
        }
        self.objects.values().any(|obj| {
            obj.is_alive()
                && obj.is_kind_of(KindOf::Structure)
                && mine_spot_under_structure(
                    pos,
                    obj.get_position(),
                    obj.selection_radius.max(obj.thing.geometry.radius).max(1.0),
                    LAND_MINE_GEOMETRY_RADIUS,
                )
        })
    }

    /// Place a named land-mine template (ChinaStandardMine / ChinaClusterMine / EMP).
    pub fn place_land_mine_named(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::LandMine,
            template_name,
            team,
            position,
            producer,
            None,
            None,
        )
    }

    /// C++ GenerateMinefieldBehavior upgradeImplementation + EMP swap update().
    pub(in super::super) fn apply_structure_minefield_upgrade(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> u32 {
        use crate::game_logic::host_mines::{
            HostMineKind, china_mine_template_for_upgrade, is_china_emp_mines_upgrade,
            is_china_mines_upgrade, structure_minefield_positions_for_geom,
        };
        if !is_china_mines_upgrade(upgrade) {
            return 0;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return 0;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return 0;
        }
        let team = obj.team;
        let pos = obj.get_position();
        let major = obj
            .selection_radius
            .max(obj.thing.geometry.radius)
            .max(((obj.thing.geometry.bounds_max.x - obj.thing.geometry.bounds_min.x).abs()) * 0.5)
            .max(1.0);
        let minor = obj
            .thing
            .geometry
            .radius
            .max(((obj.thing.geometry.bounds_max.z - obj.thing.geometry.bounds_min.z).abs()) * 0.5)
            .max(1.0);
        let is_box = (major - minor).abs() > 1.0;
        let template = china_mine_template_for_upgrade(upgrade);

        if is_china_emp_mines_upgrade(upgrade) {
            let old: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, o)| {
                    let md = o.mine_data.as_ref()?;
                    if o.is_alive()
                        && matches!(md.kind, HostMineKind::LandMine)
                        && (md.producer_id == Some(object_id) || o.producer_id == Some(object_id))
                    {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for id in old {
                self.mark_object_for_destruction(id, None);
                if let Some(o) = self.objects.get_mut(&id) {
                    if let Some(md) = o.mine_data.as_mut() {
                        md.detonated = true;
                    }
                }
            }
        } else {
            let already = self.objects.values().any(|o| {
                o.is_alive()
                    && o.mine_data.as_ref().is_some_and(|md| {
                        matches!(md.kind, HostMineKind::LandMine)
                            && (md.producer_id == Some(object_id)
                                || o.producer_id == Some(object_id))
                    })
            });
            if already {
                return 0;
            }
        }

        let spots = structure_minefield_positions_for_geom(pos, major, minor, is_box);
        let mut placed = 0u32;
        for spot in spots {
            if self.mine_spot_blocked(spot) {
                continue;
            }
            if self
                .place_land_mine_named(template, team, spot, Some(object_id))
                .is_some()
            {
                placed = placed.saturating_add(1);
            }
        }
        self.structure_minefield_placements =
            self.structure_minefield_placements.saturating_add(placed);
        placed
    }

    /// C++ LandMineInterface::setScootParms(start, dest) after placeMineAt.
    fn apply_land_mine_scoot(&mut self, mine_id: ObjectId, start: Vec3, dest: Vec3) {
        let ground_y = self.terrain_height_at(dest).unwrap_or(dest.y);
        let Some(obj) = self.objects.get_mut(&mine_id) else {
            return;
        };
        let Some(md) = obj.mine_data.as_mut() else {
            return;
        };
        if !matches!(
            md.kind,
            crate::game_logic::host_mines::HostMineKind::LandMine
        ) {
            return;
        }
        let spawn = md.set_scoot_parms(start, dest, ground_y);
        obj.set_position(spawn);
    }

    /// C++ MinefieldBehavior::update scoot integrate.
    fn update_land_mine_scoot(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                let md = o.mine_data.as_ref()?;
                if md.is_scooting() && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let ground_y = self.terrain_height_at(pos).unwrap_or(pos.y);
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if let Some(next) = md.tick_scoot(pos, ground_y) {
                        obj.set_position(next);
                    }
                }
            }
        }
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
            BURTON_UNIQUE_CHARGE_TARGETS, HostMineData, HostMineKind, can_place_remote_charge,
            can_place_timed_charge,
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
            crate::game_logic::host_mines::HostMineKind::LandMine => {
                HostMineData::land_mine_for_template(template_name)
            }
            crate::game_logic::host_mines::HostMineKind::DemoTrap => HostMineData::demo_trap(),
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge => {
                let mut d = HostMineData::timed_demo_charge(self.frame);
                if let Some(delay) = delay_frames {
                    d = d.with_lifetime_frames(self.frame, delay);
                }
                d
            }
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                let mut d = HostMineData::remote_demo_charge();
                d.next_ping_frame = Some(
                    self.frame
                        .saturating_add(crate::game_logic::host_mines::UNIT_BOMB_PING_FRAMES),
                );
                d
            }
        };
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(t) = attach_to {
            data = data.with_attach(t);
        }

        let planter_owner =
            producer.and_then(|pid| self.objects.get(&pid).and_then(|o| o.owner_player_id));
        let producer_pos =
            producer.and_then(|pid| self.objects.get(&pid).map(|o| o.get_position()));
        let dest_ground = self.terrain_height_at(position).unwrap_or(position.y);
        let sink_to_planter = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            // C++ SpecialAbilityUpdate::createSpecialObject setExperienceSink(planter).
            // Charges are untrainable; without a sink scoreTheKill drops the XP.
            if sink_to_planter {
                if let Some(p) = producer {
                    obj.set_experience_sink(Some(p));
                }
                if let Some(owner) = planter_owner {
                    obj.owner_player_id = Some(owner);
                }
            }
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
            if matches!(kind, crate::game_logic::host_mines::HostMineKind::LandMine) {
                if let Some(start) = producer_pos {
                    if let Some(md) = obj.mine_data.as_mut() {
                        let spawn = md.set_scoot_parms(start, position, dest_ground);
                        obj.set_position(spawn);
                    }
                }
            }
        }

        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(kind.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        if matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        ) {
            // C++ StickyBombUpdate::initStickyBomb: getPerUnitSound("StickyBombCreated") at bomb pos.
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_mines::STICKY_BOMB_CREATED_AUDIO,
                false,
                true,
                None,
                155,
            );
        }
        Some(id)
    }

    /// Manually detonate a residual demo trap / charge (command residual).
    pub fn manual_detonate_mine(&mut self, mine_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::HostMineDetonateReason;
        self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual, None)
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
    /// (vehicle: ride on roof offset Z; immobile: keep plant XY, refresh ground Y).
    /// If the target dies, the charge is destroyed (C++ destroyObject(self)).
    /// Every LOGICFRAMES_PER_SECOND plays per-unit UnitBombPing.
    pub(crate) fn update_sticky_bomb_attachments(&mut self) {
        use crate::game_logic::host_mines::{
            HostMineKind, STICKY_OFFSET_Z, UNIT_BOMB_PING_AUDIO, UNIT_BOMB_PING_FRAMES,
            sticky_immobile_follow_pos, sticky_vehicle_follow_pos,
        };

        let frame = self.frame;
        let sticky_ids: Vec<(ObjectId, Option<ObjectId>)> = self
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
                Some((*id, md.attached_to))
            })
            .collect();

        let mut destroy_charges: Vec<ObjectId> = Vec::new();
        let mut moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();
        let mut pings: Vec<(ObjectId, glam::Vec3)> = Vec::new();

        for (charge_id, target_id) in sticky_ids {
            if let Some(target_id) = target_id {
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
                let charge_pos = self
                    .objects
                    .get(&charge_id)
                    .map(|c| c.get_position())
                    .unwrap_or(tpos);
                let ground_y = self
                    .terrain
                    .as_ref()
                    .map(|t| t.height_at_world(charge_pos))
                    .unwrap_or(0.0);
                let new_pos = if immobile {
                    // C++ IMMOBILE: keep bomber plant XY, only refresh Z/Y to terrain.
                    sticky_immobile_follow_pos(charge_pos, ground_y)
                } else {
                    sticky_vehicle_follow_pos(tpos, STICKY_OFFSET_Z)
                };
                moves.push((charge_id, new_pos));
            }

            if let Some(md) = self
                .objects
                .get(&charge_id)
                .and_then(|o| o.mine_data.as_ref())
            {
                if md.next_ping_frame.is_some_and(|at| frame >= at) {
                    let pos = self
                        .objects
                        .get(&charge_id)
                        .map(|o| o.get_position())
                        .unwrap_or(Vec3::ZERO);
                    pings.push((charge_id, pos));
                }
            }
        }

        for (charge_id, pos) in moves {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                obj.set_position(pos);
            }
            self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
        }
        for (charge_id, _pos) in pings {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let next = md.next_ping_frame.unwrap_or(frame);
                    md.next_ping_frame = Some(
                        next.saturating_add(UNIT_BOMB_PING_FRAMES)
                            .max(frame.saturating_add(UNIT_BOMB_PING_FRAMES)),
                    );
                }
            }
            // C++ StickyBombUpdate::update: getPerUnitSound("UnitBombPing") + setObjectID.
            self.queue_resolved_per_unit_sound(
                charge_id,
                UNIT_BOMB_PING_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
        for charge_id in destroy_charges {
            self.sticky_bomb_target_deaths = self.sticky_bomb_target_deaths.saturating_add(1);
            self.destroy_object(charge_id);
        }
    }

    fn capture_booby_plant_xy(&mut self) {
        let snaps: Vec<(ObjectId, ObjectId)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?)))
            .collect();
        for (sid, cid) in snaps {
            if let Some(pos) = self.objects.get(&cid).map(|o| o.get_position()) {
                self.booby_trap.capture_plant_xy_if_unset(sid, pos.x, pos.z);
            }
        }
    }

    fn restore_booby_plant_xy_and_ping(&mut self) {
        use crate::game_logic::host_mines::{UNIT_BOMB_PING_AUDIO, UNIT_BOMB_PING_FRAMES};
        let frame = self.frame;
        let jobs: Vec<(ObjectId, ObjectId)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?)))
            .collect();
        for (sid, cid) in jobs {
            let ground_y = self
                .objects
                .get(&cid)
                .map(|o| {
                    self.terrain
                        .as_ref()
                        .map(|t| t.height_at_world(o.get_position()))
                        .unwrap_or(0.0)
                })
                .unwrap_or(0.0);
            if let Some(pos) = self.booby_trap.immobile_follow_pos(sid, ground_y) {
                if let Some(obj) = self.objects.get_mut(&cid) {
                    obj.set_position(pos);
                }
            }
        }
        let ping_ids: Vec<(ObjectId, ObjectId, u32)> = self
            .booby_trap
            .plants()
            .filter_map(|p| Some((p.structure_id, p.charge_object_id?, p.next_ping_frame?)))
            .collect();
        let mut due = Vec::new();
        for (sid, cid, at) in ping_ids {
            if frame < at {
                continue;
            }
            if let Some(pos) = self.objects.get(&cid).map(|o| o.get_position()) {
                due.push((sid, cid, pos));
            }
        }
        for (sid, cid, _pos) in due {
            if let Some(p) = self.booby_trap.plant_mut(sid) {
                p.next_ping_frame = Some(frame.saturating_add(UNIT_BOMB_PING_FRAMES));
            }
            self.queue_resolved_per_unit_sound(cid, UNIT_BOMB_PING_AUDIO, true, false, None, 140);
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
                if owner_dead { Some(*id) } else { None }
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

    /// C++ MinefieldBehavior::update producer-death drain + AutoHeal + onDamage virtual sync.
    fn update_land_mine_regen_and_virtual(&mut self) {
        use crate::game_logic::host_enum_table_residual::rubble_model_bit;
        use crate::game_logic::host_mines::{
            HostMineKind, MINE_CREATOR_DEATH_CHECK_FRAMES, MineOnDamageStep,
        };

        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                let md = o.mine_data.as_ref()?;
                if md.detonated || !matches!(md.kind, HostMineKind::LandMine) {
                    return None;
                }
                // C++ onDamage runs after HP hits 0 and chain-detonates remaining
                // virtuals before destroyObject. Do not drop lethal pads here.
                if !o.is_alive() && !md.defers_lethal_body_destroy() {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut detonate: Vec<(ObjectId, u32)> = Vec::new();
        let mut destroy: Vec<ObjectId> = Vec::new();

        for id in ids {
            let producer = self.objects.get(&id).and_then(|o| {
                o.mine_data
                    .as_ref()
                    .and_then(|m| m.producer_id)
                    .or(o.producer_id)
            });
            let producer_dead = match producer {
                Some(pid) => self
                    .objects
                    .get(&pid)
                    .map(|p| !p.is_alive() || p.status.effectively_dead)
                    .unwrap_or(true),
                None => false,
            };

            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
            let mut health = obj.health.current;
            let Some(md) = obj.mine_data.as_mut() else {
                continue;
            };

            if md.regenerates
                && md.stops_regen_after_creator_dies
                && producer_dead
                && frame >= md.next_death_check_frame
            {
                md.note_producer_dead(frame);
            } else if md.regenerates
                && md.stops_regen_after_creator_dies
                && frame >= md.next_death_check_frame
            {
                md.next_death_check_frame = frame.saturating_add(MINE_CREATOR_DEATH_CHECK_FRAMES);
            }

            let mut healing = false;
            let mut self_drain = false;
            if md.draining {
                let amount = md.drain_health_amount(max_h);
                if amount > 0.0 {
                    health = (health - amount).max(0.0);
                    self_drain = true;
                }
            } else {
                let healed = md.tick_regen_auto_heal(frame, health, max_h);
                if healed > health + 1e-4 {
                    health = healed;
                    healing = true;
                }
            }

            if md
                .last_synced_health
                .is_some_and(|h| (h - health).abs() < 1e-4)
                && !self_drain
                && !healing
            {
                continue;
            }

            let mut want_detonate_n = 0u32;
            let mut want_destroy = false;
            loop {
                match md.apply_on_damage_step(health, max_h, healing, self_drain) {
                    MineOnDamageStep::Done => break,
                    MineOnDamageStep::Continue => {
                        if md.virtual_mines_remaining == 0 && !md.regenerates {
                            want_destroy = true;
                            break;
                        }
                    }
                    MineOnDamageStep::Detonate => {
                        let expected =
                            md.virtual_mines_expected_from_health(health, max_h, healing);
                        want_detonate_n = md.virtual_mines_remaining.saturating_sub(expected);
                        break;
                    }
                }
            }

            health = md.clamp_empty_regen_health(health);
            let empty = md.virtual_mines_remaining == 0;
            let regenerates = md.regenerates;
            drop(md);
            obj.health.current = health;
            let bit = 1u128 << rubble_model_bit();
            if empty {
                obj.model_condition_bits |= bit;
                obj.set_status_masked(true);
                if !regenerates && health <= 0.0 {
                    want_destroy = true;
                }
            } else {
                obj.model_condition_bits &= !bit;
                obj.set_status_masked(false);
            }
            if want_detonate_n > 0 {
                detonate.push((id, want_detonate_n));
            }
            if want_destroy {
                destroy.push(id);
            }
        }

        for (id, n) in detonate {
            let pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            for _ in 0..n {
                if !self.trip_virtual_land_mine(id, pos) {
                    break;
                }
            }
            // C++ onDamage after the loop: regen pads at 0 HP restore MIN_HEALTH.
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let clamped = md.clamp_empty_regen_health(obj.health.current);
                    obj.health.current = clamped;
                    md.last_synced_health = Some(clamped);
                    let empty = md.virtual_mines_remaining == 0;
                    let regenerates = md.regenerates;
                    drop(md);
                    let bit = 1u128 << rubble_model_bit();
                    if empty {
                        obj.model_condition_bits |= bit;
                        obj.set_status_masked(true);
                        if !regenerates && clamped <= 0.0 {
                            destroy.push(id);
                        }
                    } else {
                        obj.model_condition_bits &= !bit;
                        obj.set_status_masked(false);
                    }
                }
            }
        }

        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn update_mines_and_demo_traps(&mut self) {
        use crate::game_logic::host_mines::{
            DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE, HostMineDetonateReason,
            HostMineKind, can_clear_mine_kind, demo_trap_skips_dozer_disarm_while_attacking,
            is_mine_clearer, land_mine_geometry_contacts, mine_clear_allowed_for_order,
            minefield_skips_worker, victim_mine_collide_radius,
        };

        let frame = self.frame;
        for id in crate::game_logic::host_status_log::drain_under_construction_mine_sweeps() {
            self.sweep_under_construction_footprint_mines(id);
        }

        self.update_land_mine_scoot();

        self.update_land_mine_regen_and_virtual();

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
            self.capture_booby_plant_xy();
            self.update_booby_trap_special_attachments();
            self.restore_booby_plant_xy_and_ping();
        }
        // C++ SpecialObjectsPersistWhenOwnerDies = No for RemoteC4Charge residual.
        self.cleanup_remote_charges_when_owner_dies();
        let mut due: Vec<(ObjectId, HostMineDetonateReason)> = Vec::new();
        let mut clear_due: Vec<(ObjectId, ObjectId)> = Vec::new(); // (mine_id, clearer_id)
        let mut approach: Vec<(ObjectId, Vec3)> = Vec::new(); // clearer moves toward mine
        // Clearers inside clear range winding up PreAttackDelay (not yet clear_due).
        // C++ isClearingMines() && getGoalObject() != NULL keeps them immune while engaged.
        let mut windup: Vec<(ObjectId, ObjectId)> = Vec::new();
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
                // C++ DemoTrapUpdate::update returns on OBJECT_STATUS_SOLD.
                if obj.status.sold {
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
        let victims: Vec<(ObjectId, Team, Vec3, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                let is_dozer = is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name);
                let weapon_disarm = obj
                    .weapon_name_for_slot(0)
                    .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                    .unwrap_or(false)
                    || obj.weapon_set_mine_clearing_detail;
                let attacking = obj.status.attacking || matches!(obj.ai_state, AIState::Attacking);
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
                let ground_y = self.terrain_height_at(obj.get_position()).unwrap_or(0.0);
                if crate::game_logic::host_mines::is_above_terrain(obj.get_position().y, ground_y) {
                    return None;
                }
                let geom = &obj.thing.template.geometry_info;
                let victim_r = victim_mine_collide_radius(
                    geom.authored,
                    geom.bounding_circle_radius(),
                    obj.selection_radius,
                );
                Some((*id, obj.team, obj.get_position(), victim_r))
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
                    let delay = self
                        .objects
                        .get(cid)
                        .map(|o| {
                            crate::game_logic::host_mines::mine_clear_pre_attack_frames(
                                o.is_kind_of(KindOf::Worker),
                                &o.template_name,
                            )
                        })
                        .unwrap_or(
                            crate::game_logic::host_mines::DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES,
                        );
                    let ready = self
                        .objects
                        .get_mut(&mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.begin_or_ready_clear_pre_attack(*cid, frame, delay));
                    if !clear_due.iter().any(|(m, _)| *m == mine_id) {
                        if ready {
                            clear_due.push((mine_id, *cid));
                        } else {
                            windup.push((mine_id, *cid));
                        }
                    }
                } else {
                    if let Some(obj) = self.objects.get_mut(&mine_id) {
                        if let Some(md) = obj.mine_data.as_mut() {
                            if md.clear_pre_attack_clearer == Some(*cid) {
                                md.reset_clear_pre_attack();
                            }
                        }
                    }
                    // Approach residual: move idle clearer toward nearest mine.
                    approach.push((*cid, mine_pos));
                }
            }
        }

        // C++ MinefieldBehavior::update expire immunities (collideTime + 2).
        for (mine_id, _, _, _, _, _, _, kind) in &mines {
            if *kind != HostMineKind::LandMine {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    md.expire_immunes(frame);
                }
            }
        }

        // Clearers with a goal mine are immune to neighboring pads (MAX_IMMUNITY).
        let mut clearing_ids: Vec<ObjectId> = Vec::new();
        for (mine_id, cid) in &clear_due {
            clearing_ids.push(*cid);
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if matches!(md.kind, HostMineKind::LandMine) {
                        md.grant_immunity(*cid, frame);
                    }
                }
            }
        }
        for (mine_id, cid) in &windup {
            clearing_ids.push(*cid);
            if let Some(obj) = self.objects.get_mut(mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if matches!(md.kind, HostMineKind::LandMine) {
                        md.grant_immunity(*cid, frame);
                    }
                }
            }
        }
        for (cid, mine_pos) in &approach {
            clearing_ids.push(*cid);
            // Grant immunity on every land mine the clearer is currently overlapping.
            for (mine_id, _, mpos, trigger, .., kind) in &mines {
                if *kind != HostMineKind::LandMine {
                    continue;
                }
                let dx = mine_pos.x - mpos.x;
                let dz = mine_pos.z - mpos.z;
                let r = trigger.max(DOZER_MINE_CLEAR_SCAN_RANGE);
                if dx * dx + dz * dz <= r * r {
                    if let Some(obj) = self.objects.get_mut(mine_id) {
                        if let Some(md) = obj.mine_data.as_mut() {
                            md.grant_immunity(*cid, frame);
                        }
                    }
                }
            }
        }

        let mut land_trips: Vec<(ObjectId, ObjectId, Vec3)> = Vec::new();
        for (
            mine_id,
            _mine_team,
            mine_pos,
            trigger_range,
            proximity,
            detonate_at,
            under_construction,
            kind,
        ) in &mines
        {
            if *under_construction {
                continue;
            }
            // C++ MinefieldBehavior::onCollide :360-362 — inert until scoot ends.
            if self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.is_scooting())
            {
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
            let warning_ready = self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.demo_trap_warning_ready(frame));
            if warning_ready {
                due.push((*mine_id, HostMineDetonateReason::Proximity));
                continue;
            }
            if self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .is_some_and(|md| md.demo_trap_warning_armed())
            {
                continue;
            }
            if !proximity || *trigger_range <= 0.0 {
                continue;
            }
            let range_sqr = trigger_range * trigger_range;
            let workers_detonate = self
                .objects
                .get(mine_id)
                .and_then(|o| o.mine_data.as_ref())
                .map(|m| m.workers_detonate)
                .unwrap_or(false);
            for (vid, _vteam, vpos, vgeom) in &victims {
                if *vid == *mine_id {
                    continue;
                }
                if self.mine_proximity_skips_friendly(*mine_id, *vid) {
                    continue;
                }
                // Land mines: C++ MinefieldBehavior::onCollide (geometry overlap).
                // Demo traps: C++ DemoTrapUpdate FROM_CENTER_2D trigger range.
                let in_contact = if *kind == HostMineKind::LandMine {
                    land_mine_geometry_contacts(*mine_pos, *trigger_range, *vpos, *vgeom)
                } else {
                    let dx = vpos.x - mine_pos.x;
                    let dz = vpos.z - mine_pos.z;
                    dx * dx + dz * dz <= range_sqr
                };
                if !in_contact {
                    continue;
                }
                if *kind == HostMineKind::LandMine {
                    let Some(victim) = self.objects.get(vid) else {
                        continue;
                    };
                    if minefield_skips_worker(
                        workers_detonate,
                        victim.is_kind_of(KindOf::Infantry),
                        victim.is_kind_of(KindOf::Dozer),
                    ) {
                        continue;
                    }
                    if clearing_ids.contains(vid) {
                        if let Some(obj) = self.objects.get_mut(mine_id) {
                            if let Some(md) = obj.mine_data.as_mut() {
                                md.grant_immunity(*vid, frame);
                            }
                        }
                        continue;
                    }
                    let immune = self
                        .objects
                        .get_mut(mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.refresh_immune(*vid, frame));
                    if immune {
                        continue;
                    }
                    let allow = self
                        .objects
                        .get_mut(mine_id)
                        .and_then(|o| o.mine_data.as_mut())
                        .is_some_and(|md| md.allow_repeat_detonate(*vid, *vpos));
                    if !allow {
                        continue;
                    }
                    land_trips.push((*mine_id, *vid, *vpos));
                    break;
                }
                due.push((*mine_id, HostMineDetonateReason::Proximity));
                break;
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

        for (mine_id, _victim, vpos) in land_trips {
            let _ = self.trip_virtual_land_mine(mine_id, vpos);
        }

        for (mine_id, reason) in due {
            let _ = self.detonate_mine_internal(mine_id, reason, None);
        }
    }

    /// C++ MinefieldBehavior::detonateOnce — one virtual mine, pad may persist.
    fn trip_virtual_land_mine(&mut self, mine_id: ObjectId, victim_pos: Vec3) -> bool {
        use crate::game_logic::host_enum_table_residual::rubble_model_bit;
        use crate::game_logic::host_mines::{
            LAND_MINE_GEOMETRY_RADIUS, MINE_MIN_HEALTH, clip_point_to_mine_footprint,
        };

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        // C++ detonateOnce from onDamage while the pad may already be at 0 HP.
        let allow_zero_hp = mine
            .mine_data
            .as_ref()
            .is_some_and(|md| md.defers_lethal_body_destroy());
        if !mine.is_alive() && !allow_zero_hp {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if !data.is_active()
            || !matches!(
                data.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            )
        {
            return false;
        }
        let mine_pos = mine.get_position();
        let blast_pos =
            clip_point_to_mine_footprint(mine_pos, victim_pos, LAND_MINE_GEOMETRY_RADIUS);
        let destroy_pad = {
            let Some(obj) = self.objects.get_mut(&mine_id) else {
                return false;
            };
            let Some(md) = obj.mine_data.as_mut() else {
                return false;
            };
            md.consume_virtual_mine()
        };
        if destroy_pad {
            return self.detonate_mine_internal(
                mine_id,
                crate::game_logic::host_mines::HostMineDetonateReason::Proximity,
                Some(blast_pos),
            );
        }

        // Persist: apply one blast, keep the pad, scale health, rubble if empty.
        self.mine_residual_proximity_detonations =
            self.mine_residual_proximity_detonations.saturating_add(1);
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            let desired = obj
                .mine_data
                .as_ref()
                .map(|md| (md.virtual_health_fraction() * obj.health.maximum).max(MINE_MIN_HEALTH))
                .unwrap_or(MINE_MIN_HEALTH);
            let empty = obj
                .mine_data
                .as_ref()
                .is_some_and(|md| md.virtual_mines_remaining == 0);
            if obj.health.current > desired {
                obj.health.current = desired;
            }
            let hp = obj.health.current;
            if let Some(md) = obj.mine_data.as_mut() {
                md.last_synced_health = Some(hp);
            }
            let bit = 1u128 << rubble_model_bit();
            if empty {
                obj.model_condition_bits |= bit;
                obj.set_status_masked(true);
            } else {
                obj.model_condition_bits &= !bit;
                obj.set_status_masked(false);
            }
        }
        let (damage, radius, kind, mine_team, producer) = {
            let obj = self.objects.get(&mine_id).unwrap();
            let md = obj.mine_data.as_ref().unwrap();
            (
                md.detonation_damage,
                md.detonation_radius,
                md.kind,
                obj.team,
                md.producer_id,
            )
        };
        self.apply_mine_splash(
            mine_id, blast_pos, damage, radius, kind, mine_team, producer,
        );
        true
    }

    /// Safely disarm/clear a residual mine without detonation or area damage.
    /// C++ Weapon DAMAGE_DISARM → LandMineInterface::disarm / destroyObject residual.
    /// Demo traps (KINDOF_DEMOTRAP) are disarmed the same way — no explosion.
    pub fn clear_mine_internal(&mut self, mine_id: ObjectId, clearer_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::{MINE_CLEARED_AUDIO, can_clear_mine_kind};

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
            return false;
        }
        let mine_pos = mine.get_position();
        let keep_regen_pad = data.regenerates
            && matches!(
                data.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            );

        if keep_regen_pad {
            use crate::game_logic::host_enum_table_residual::rubble_model_bit;
            use crate::game_logic::host_mines::MINE_MIN_HEALTH;
            if let Some(obj) = self.objects.get_mut(&mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let _ = md.disarm_regenerating_pad();
                }
                obj.health.current = MINE_MIN_HEALTH;
                obj.model_condition_bits |= 1u128 << rubble_model_bit();
                obj.set_status_masked(true);
            }
        } else if let Some(obj) = self.objects.get_mut(&mine_id) {
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

        if !keep_regen_pad {
            self.mark_object_for_destruction(mine_id, None);
        }

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
        // C++ MinefieldBehavior::detonateOnce(position): onCollide fires the
        // detonation weapon at the clipped victim point (MinefieldBehavior.cpp:257,
        // :422-424); DemoTrapUpdate::detonate / onDamage pass the object position.
        blast_at: Option<Vec3>,
    ) -> bool {
        use crate::game_logic::host_mines::{HostMineDetonateReason, damage_at_distance};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if mine.status.sold {
            return false;
        }
        if !mine.is_alive() && !matches!(reason, HostMineDetonateReason::Killed) {
            // Last detonateOnce of a 0-HP non-regen pad still fires + destroyObject.
            let last_virtual = mine.mine_data.as_ref().is_some_and(|md| {
                matches!(
                    md.kind,
                    crate::game_logic::host_mines::HostMineKind::LandMine
                ) && !md.detonated
            });
            if !last_virtual {
                return false;
            }
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
        let neutron_blast = data.neutron_blast;
        let mine_team = mine.team;
        let mine_pos = mine.get_position();
        let producer = data.producer_id;
        let frame = self.frame;
        let blast_pos = blast_at.unwrap_or(mine_pos);

        // C++ DemoTrapUpdate.cpp:136-141, 240-251: selecting the detonation
        // weapon slot (command button) calls detonate() which fires the
        // detonation weapon and kills the trap the SAME frame — no SlowDeath
        // warning arm. Only passive paths (proximity scan / death) ride the
        // SlowDeath DestructionDelay residual.
        if is_demo_trap
            && reason != crate::game_logic::host_mines::HostMineDetonateReason::Manual
            && !data.demo_trap_warning_ready(frame)
        {
            if let Some(obj) = self.objects.get_mut(&mine_id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    let _ = md.arm_demo_trap_warning(frame);
                }
            }
            self.queue_audio_event(
                AudioEventRequest::new(crate::game_logic::host_mines::DEMO_TRAP_WARNING_AUDIO)
                    .with_object(mine_id)
                    .with_position(mine_pos)
                    .with_priority(180),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponImpact,
                mine_pos,
                frame,
                Some(mine_id),
                None,
            );
            return true;
        }

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
        let mut chain_ids: Vec<ObjectId> = Vec::new();
        if neutron_blast {
            destroy_ids = self.apply_neutron_mine_blast(mine_id, blast_pos, mine_team);
        } else {
            let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
            for vid in victim_ids {
                if vid == mine_id {
                    continue;
                }
                let Some(victim) = self.objects.get(&vid) else {
                    continue;
                };
                if !victim.is_alive() {
                    continue;
                }
                let chain_pad = land_mine_pad_takes_splash(victim);
                if victim.mine_data.is_some() && !chain_pad {
                    continue;
                }
                if victim.team == mine_team && !hit_allies && !chain_pad {
                    continue;
                }
                let dist = mine_splash_distance(victim, blast_pos);
                let dmg = if is_demo_trap {
                    crate::game_logic::host_mines::demo_trap_damage_at(demo_profile, dist)
                } else {
                    damage_at_distance(damage, radius, dist)
                };
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(victim) = self.objects.get_mut(&vid) {
                    // C++ DemoTrap/LandMine weapons are EXPLOSION, not UNRESISTABLE.
                    if victim.take_damage_from_typed(
                        dmg,
                        Some(mine_id),
                        crate::game_logic::combat::DamageType::Explosive,
                    ) && !chain_pad
                    {
                        destroy_ids.push((vid, mine_team));
                    } else if chain_pad {
                        chain_ids.push(vid);
                    }
                }
            }
            for id in chain_ids {
                self.chain_detonate_land_mine_from_splash(id);
            }
        }

        // Chem DemoTrap residual: spawn MediumPoisonField at detonation.
        if is_demo_trap && demo_profile.spawns_poison() {
            let _ = self.toxin_tractor.spawn_medium_field(
                mine_id,
                mine_team,
                blast_pos,
                self.frame,
                demo_profile.poison_anthrax_tier(),
            );
        }

        // Audio + particle residual.
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(blast_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            blast_pos,
            self.frame,
            Some(mine_id),
            None,
        );

        // Destroy the mine/trap itself.
        self.mark_object_for_destruction(mine_id, producer.map(|_| mine_team));
        for (vid, killer) in destroy_ids {
            // C++ scoreTheKill on the charge; addExperiencePoints sinks to planter.
            self.award_score_the_kill_experience(mine_id, vid);
            self.mark_object_for_destruction(vid, Some(killer));
        }

        true
    }

    fn apply_mine_splash(
        &mut self,
        mine_id: ObjectId,
        blast_pos: Vec3,
        damage: f32,
        radius: f32,
        kind: crate::game_logic::host_mines::HostMineKind,
        mine_team: Team,
        _producer: Option<ObjectId>,
    ) {
        use crate::game_logic::host_mines::damage_at_distance;
        let neutron_blast = self
            .objects
            .get(&mine_id)
            .and_then(|o| o.mine_data.as_ref())
            .is_some_and(|md| md.neutron_blast);
        if neutron_blast {
            let destroy_ids = self.apply_neutron_mine_blast(mine_id, blast_pos, mine_team);
            self.queue_audio_event(
                AudioEventRequest::new(kind.detonate_audio())
                    .with_object(mine_id)
                    .with_position(blast_pos)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponImpact,
                blast_pos,
                self.frame,
                Some(mine_id),
                None,
            );
            for (vid, killer) in destroy_ids {
                self.award_score_the_kill_experience(mine_id, vid);
                self.mark_object_for_destruction(vid, Some(killer));
            }
            return;
        }
        let hit_allies = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::DemoTrap
                | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let mut chain_ids: Vec<ObjectId> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == mine_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() {
                continue;
            }
            let chain_pad = land_mine_pad_takes_splash(victim);
            if victim.mine_data.is_some() && !chain_pad {
                continue;
            }
            if victim.team == mine_team && !hit_allies && !chain_pad {
                continue;
            }
            let dist = mine_splash_distance(victim, blast_pos);
            let dmg = damage_at_distance(damage, radius, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                if victim.take_damage_from(dmg, Some(mine_id)) && !chain_pad {
                    destroy_ids.push((vid, mine_team));
                } else if chain_pad {
                    chain_ids.push(vid);
                }
            }
        }
        for id in chain_ids {
            self.chain_detonate_land_mine_from_splash(id);
        }
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(blast_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            blast_pos,
            self.frame,
            Some(mine_id),
            None,
        );
        for (vid, killer) in destroy_ids {
            self.award_score_the_kill_experience(mine_id, vid);
            self.mark_object_for_destruction(vid, Some(killer));
        }
    }

    /// C++ MinefieldBehavior::onDamage after splash `attemptDamage`.
    fn chain_detonate_land_mine_from_splash(&mut self, mine_id: ObjectId) {
        let Some(obj) = self.objects.get(&mine_id) else {
            return;
        };
        let Some(md) = obj.mine_data.as_ref() else {
            return;
        };
        if md.detonated
            || !matches!(
                md.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            )
        {
            return;
        }
        let health = obj.health.current;
        let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
        let expected = md.virtual_mines_expected_from_health(health, max_h, false);
        let n = md.virtual_mines_remaining.saturating_sub(expected);
        if n == 0 {
            return;
        }
        let pos = obj.get_position();
        for _ in 0..n {
            if !self.trip_virtual_land_mine(mine_id, pos) {
                break;
            }
        }
    }
    fn apply_neutron_mine_blast(
        &mut self,
        mine_id: ObjectId,
        blast_pos: Vec3,
        mine_team: Team,
    ) -> Vec<(ObjectId, Team)> {
        use crate::game_logic::host_mines::{NeutronMineVictimEffect, neutron_mine_victim_effect};
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let mut contained_kill: Vec<ObjectId> = Vec::new();
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
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - blast_pos.x;
                let dz = vpos.z - blast_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            let airborne = victim.status.airborne_target
                || crate::game_logic::host_mines::is_above_terrain(
                    vpos.y,
                    self.terrain_height_at(vpos).unwrap_or(0.0),
                );
            let effect = neutron_mine_victim_effect(
                dist,
                victim.team == mine_team,
                airborne,
                victim.is_kind_of(KindOf::Infantry),
                victim.is_kind_of(KindOf::Vehicle),
                victim.is_kind_of(KindOf::Drone),
                victim.template_name.to_ascii_lowercase().contains("bike"),
            );
            match effect {
                NeutronMineVictimEffect::None => {}
                NeutronMineVictimEffect::KillInfantry
                | NeutronMineVictimEffect::KillCliffJumper => {
                    contained_kill.extend(victim.contained_units());
                    destroy_ids.push((vid, mine_team));
                }
                NeutronMineVictimEffect::UnmanVehicle => {
                    contained_kill.extend(victim.contained_units());
                    if let Some(victim) = self.objects.get_mut(&vid) {
                        victim.set_status_disabled_unmanned(true);
                        victim.team = Team::Neutral;
                    }
                }
            }
        }
        for cid in contained_kill {
            if let Some(c) = self.objects.get_mut(&cid) {
                if c.is_alive() {
                    destroy_ids.push((cid, mine_team));
                }
            }
        }
        destroy_ids
    }

    /// C++ `Object::getRelationship`. DemoTrap: ENEMIES only.
    /// Land mines: DetonatedBy ENEMIES | NEUTRAL (ctor default).
    fn mine_proximity_skips_friendly(&self, mine_id: ObjectId, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::{
            HostMineKind, demo_trap_proximity_requires_enemies, land_mine_proximity_trips,
        };
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
        let is_land = mine
            .mine_data
            .as_ref()
            .is_some_and(|md| matches!(md.kind, HostMineKind::LandMine));
        if is_land {
            !land_mine_proximity_trips(rel == Relationship::Enemies, rel == Relationship::Neutral)
        } else {
            !demo_trap_proximity_requires_enemies(rel == Relationship::Enemies)
        }
    }

    /// C++ `StealthUpdate::markAsDetected` (`StealthUpdate.cpp:892-910`) +
    /// `setWakeupIfInRange` / `wakeUpAndAttemptToTarget`.
    ///
    /// Object has no world access, so the idle-enemy walk lives here. Retail
    /// Burton / Kell / Lotus / Pathfinder / Listening Outpost / CamoNetting
    /// author `OrderIdleEnemiesToAttackMeUponReveal`. Idle enemy AI in vision
    /// gets `m_nextMoodCheckTime = now` so same-frame mood acquire can fire.
    pub(crate) fn order_idle_enemies_to_attack_on_reveal(&mut self, victim_id: ObjectId) {
        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        if !crate::game_logic::object::order_idle_enemies_on_reveal(&victim.template_name) {
            return;
        }
        let v_team = victim.team;
        let v_owner = victim.owner_player_id;
        let v_pos = victim.get_position();
        let now = self.frame;

        let idle_in_vision: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&id, o)| {
                if id == victim_id || !o.is_alive() {
                    return None;
                }
                // C++ `setWakeupIfInRange` requires `getAI()`.
                if matches!(o.object_type, ObjectType::Projectile) {
                    return None;
                }
                // C++ Object::getRelationship (Object.cpp:1548-1568) falls through
                // to the controlling players' team relationship: two living
                // players on different non-neutral teams are ENEMIES even when
                // the lobby carries no explicit playerEnemies row. A strict
                // player-map NEUTRAL must not hard-reject the wake.
                let mut enemy = match (o.owner_player_id, v_owner) {
                    (Some(a), Some(b)) => {
                        self.player_relationship(a, b) == gamelogic::common::Relationship::Enemies
                    }
                    _ => o.team != v_team && o.team != Team::Neutral && v_team != Team::Neutral,
                };
                if !enemy
                    && o.owner_player_id.is_some()
                    && v_owner.is_some()
                    && o.team != Team::Neutral
                    && v_team != Team::Neutral
                    && o.team != v_team
                {
                    enemy = true;
                }
                if !enemy {
                    return None;
                }
                let vision = if o.vision_range > 0.0 {
                    o.vision_range
                } else {
                    o.get_template().sight_range.max(0.0)
                };
                if vision <= 0.0 || o.get_position().distance(v_pos) > vision {
                    return None;
                }
                // C++ `wakeUpAndAttemptToTarget` no-ops unless idle.
                let idle = matches!(o.ai_state, AIState::Idle) && o.target.is_none();
                idle.then_some(id)
            })
            .collect();

        for eid in idle_in_vision {
            if let Some(enemy) = self.objects.get_mut(&eid) {
                enemy.next_mood_check_time = now;
                let can_attack = enemy.weapon.is_some()
                    || enemy.is_kind_of(KindOf::Attackable)
                    || enemy.can_attack()
                    || matches!(
                        enemy.object_type,
                        ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft
                    );
                if !can_attack
                    || enemy.is_kind_of(KindOf::Structure)
                    || enemy.is_kind_of(KindOf::Worker)
                    || enemy.is_worker()
                {
                    continue;
                }
                enemy.set_target(Some(victim_id));
                enemy.set_ai_state(AIState::Attacking);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(eid, victim_id);
                    crate::game_logic::host_ai_decision_log::record_set_state(eid, 2);
                }
                self.camo_netting_order_idle_enemies_count =
                    self.camo_netting_order_idle_enemies_count.saturating_add(1);
            }
        }
    }

    /// C++ Object::setStatus UNDER_CONSTRUCTION edge (Object.cpp:985-1011).
    /// Leftover `object_status::set_status` is the reference: enemy mines
    /// `kill(LANDMINE, EXPLODED)`, ally/neutral mines silent `destroyObject`.
    pub(in super::super) fn sweep_under_construction_footprint_mines(
        &mut self,
        building_id: ObjectId,
    ) {
        use crate::game_logic::host_mines::HostMineDetonateReason;
        use gamelogic::common::Relationship;

        let Some(building) = self.objects.get(&building_id) else {
            return;
        };
        if building.status.destroyed {
            return;
        }
        let bpos = building.get_position();
        let br = if building.thing.template.geometry_info.authored {
            building
                .thing
                .template
                .geometry_info
                .bounding_circle_radius()
        } else {
            Self::structure_place_radius(building)
        }
        .max(1.0);

        let mut hits: Vec<(ObjectId, Relationship, bool)> = Vec::new();
        for (id, obj) in &self.objects {
            if *id == building_id || obj.status.destroyed {
                continue;
            }
            let is_mine = obj.is_kind_of(KindOf::Mine) || obj.mine_data.is_some();
            if !is_mine {
                continue;
            }
            let mpos = obj.get_position();
            let mr = if obj.thing.template.geometry_info.authored {
                obj.thing.template.geometry_info.bounding_circle_radius()
            } else {
                obj.selection_radius.max(1.0)
            };
            let dx = mpos.x - bpos.x;
            let dz = mpos.z - bpos.z;
            let limit = br + mr;
            if dx * dx + dz * dz > limit * limit {
                continue;
            }
            let rel = self.object_relationship(building, obj);
            hits.push((*id, rel, obj.mine_data.is_some()));
        }

        for (mine_id, rel, has_data) in hits {
            match rel {
                Relationship::Enemies => {
                    if has_data {
                        let _ = self.detonate_mine_internal(
                            mine_id,
                            HostMineDetonateReason::Proximity,
                            None,
                        );
                    } else if let Some(obj) = self.objects.get_mut(&mine_id) {
                        obj.take_damage_from_typed_death(
                            obj.health.maximum.max(1.0),
                            Some(building_id),
                            crate::game_logic::combat::DamageType::LandMine,
                            crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
                        );
                    }
                }
                Relationship::Allies | Relationship::Neutral => {
                    self.destroy_object(mine_id);
                }
            }
        }
    }
}

/// C++ `createAndFireTempWeapon` hits other `KINDOF_MINE` pads (Weapon.cpp:1283).
/// Their `MinefieldBehavior::onDamage` then `detonateOnce` until health bands match.
fn land_mine_pad_takes_splash(victim: &Object) -> bool {
    victim.mine_data.as_ref().is_some_and(|md| {
        !md.detonated
            && matches!(
                md.kind,
                crate::game_logic::host_mines::HostMineKind::LandMine
            )
            && md.has_virtual_charge()
    })
}

/// C++ `DAMAGE_RANGE_CALC_TYPE = FROM_BOUNDINGSPHERE_3D` (Weapon.cpp:70).
/// Barracks-ring pads sit ~30 wu apart with GeometryMajorRadius 30 — center
/// distance misses the r5 StructureMineWeapon, surface gap does not.
fn mine_splash_distance(victim: &Object, blast_pos: Vec3) -> f32 {
    if land_mine_pad_takes_splash(victim) {
        let sphere = crate::game_logic::combat::victim_splash_sphere_radius(victim)
            .max(crate::game_logic::host_mines::LAND_MINE_GEOMETRY_RADIUS);
        crate::game_logic::combat::splash_from_bounding_sphere_3d(
            blast_pos,
            victim.get_position(),
            sphere,
        )
    } else {
        let vpos = victim.get_position();
        let dx = vpos.x - blast_pos.x;
        let dz = vpos.z - blast_pos.z;
        (dx * dx + dz * dz).sqrt()
    }
}
