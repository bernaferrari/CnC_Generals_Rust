//! Host tick `impl GameLogic` — `shock`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

fn receiving_shock_splash_relationships(
    team_factory: &gamelogic::team::TeamFactoryHandle,
    players: &std::collections::HashMap<u32, crate::game_logic::Player>,
    source_owner: Option<u32>,
    source_team_id: Option<u32>,
    source_team_instance: &str,
    source_is_defector: bool,
    victim: &crate::game_logic::object::Object,
) -> (
    gamelogic::common::Relationship,
    gamelogic::common::Relationship,
) {
    use gamelogic::common::Relationship;

    // C++ Weapon.cpp uses source->getRelationship(victim) only for
    // DOESNT_AFFECT_SIMILAR, then reverses the query for the radius mask.
    let (similar_relationship, mask_relationship) = if source_is_defector {
        (Relationship::Neutral, Relationship::Allies)
    } else if victim.is_undetected_defector() {
        (Relationship::Allies, Relationship::Neutral)
    } else {
        (
            GameLogic::object_relationship_from_identities(
                team_factory,
                players,
                source_owner,
                source_team_id,
                source_team_instance,
                victim.owner_player_id,
                victim.team_instance_id,
                &victim.team_instance_name,
            ),
            GameLogic::object_relationship_from_identities(
                team_factory,
                players,
                victim.owner_player_id,
                victim.team_instance_id,
                &victim.team_instance_name,
                source_owner,
                source_team_id,
                source_team_instance,
            ),
        )
    };
    (similar_relationship, mask_relationship)
}

impl GameLogic {
    pub(crate) fn tick_physics_collisions_all(&mut self) -> u32 {
        self.tick_physics_collisions_all_with_effects(&mut Default::default())
            .expect("receiving Physics tick failed")
    }

    pub(crate) fn tick_physics_collisions_all_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<u32> {
        self.integrate_owned_physics_motion_with_effects(effects)?;
        Ok(self.update_owned_partition_with_effects(effects))
    }

    pub(crate) fn integrate_owned_physics_motion_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.sync_all_contained_items_mass();
        // Per-frame blocked bookkeeping residual (before new collide pairs).
        // Snapshot ground heights before mut pass (terrain borrow).
        // Only mobile / physics-active bodies need terrain samples + motion step —
        // sampling every structure on Lone Eagle (~900 objs) dominated host frames.
        let ground_heights: Vec<(ObjectId, std::sync::Weak<()>, f32)> = {
            let mut out = Vec::new();
            for (id, o) in self.objects.registered() {
                let wreck = o.status.destroyed || !o.is_alive();
                let wreck_airborne = wreck && o.get_position().y > o.ground_height + 0.05;
                let physics_active = o.can_move()
                    || o.shock_stun_frames > 0
                    || o.bounce_audio_pending > 0
                    || o.pending_ground_collide
                    || o.movement.velocity.length_squared() > 1e-6
                    || o.physics_accel.length_squared() > 1e-6
                    || o.allow_to_fall
                    || o.is_physics_held()
                    || o.motive_frames_remaining > 0
                    || (o.selected_locomotor_definition.is_some()
                        && o.has_ai_update_interface()
                        && !matches!(o.loco_behavior_z, LocomotorBehaviorZ::NoZMotiveForce))
                    || wreck_airborne;
                if !physics_active {
                    continue;
                }
                if o.is_kind_of(KindOf::Structure) && !o.allow_to_fall && !wreck_airborne {
                    continue;
                }
                let p = o.get_position();
                let terrain_y = self
                    .terrain_height_at(glam::Vec3::new(p.x, 0.0, p.z))
                    .unwrap_or(0.0);
                let layer_height = self.pathfinding_system.grid.layer_height_clipped(
                    p,
                    o.pathfind_layer,
                    terrain_y,
                );
                out.push((
                    *id,
                    std::sync::Arc::downgrade(&o.visual_draw_identity_token),
                    self.physics_ground_y_with_deck(o, layer_height),
                ));
            }
            out
        };
        let physics_ids: std::collections::HashMap<_, _> = ground_heights
            .iter()
            .map(|(id, birth, _)| (*id, birth.clone()))
            .collect();
        let mut expired_paths: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.registered_mut() {
            o.clear_blocked_frame_state();
            o.tick_move_away_state(self.locomotor_definitions.as_ref());
            if o.tick_path_queue(
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            ) {
                expired_paths.push(*id);
            }
            if physics_ids.get(id).is_some_and(|birth| {
                birth.ptr_eq(&std::sync::Arc::downgrade(&o.visual_draw_identity_token))
            }) {
                o.integrate_physics_force_phase();
            }
        }
        let snap_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| o.overlay_arrival_snap.then_some(*id))
            .collect();
        for id in snap_ids {
            if let Some(o) = self.objects.get_mut(&id) {
                o.overlay_arrival_snap = false;
            }
            self.apply_arrival_goal_snap(id, None);
        }
        self.requeue_expired_path_requests(&expired_paths);
        for (id, birth, ground_y) in ground_heights {
            if !self.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) {
                continue;
            }
            self.drive_owned_contact_with_effects(
                id,
                crate::game_logic::object::MotionContact::Start(ground_y),
                Object::step_motion_contact_with_effects,
                effects,
            )?;
            if self
                .objects
                .lookup(&id)
                .is_some_and(|o| o.pending_ground_collide)
            {
                self.dispatch_physics_ground_collide_with_effects(id, effects)?;
            }
        }
        Ok(())
    }

    /// Original PartitionManager::update: consume first-dirty order, gather
    /// contacts while each module is visited, dispatch after the list is empty.
    /// New dirtiness from contact callbacks belongs to the next phase.
    pub(crate) fn update_owned_partition_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let mut contacts = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(dirty) = self.partition_manager.take_next_receiving_dirty() {
            let id = ObjectId(dirty.object_id);
            let object = self.objects.get_mut(&id).expect(
                "attached partition dirty Object remains installed until physical retirement",
            );
            assert!(
                dirty.matches(&object.visual_draw_identity_token),
                "partition phase cannot consume a different Object lifetime"
            );
            if dirty.update_cells {
                let position = object.get_position();
                let footprint = super::collide_dispatch::host_object_footprint(object);
                self.partition_manager
                    .register_object_geometry(id.0, position.x, position.z, footprint);
                let (changed, cell) = self
                    .partition_manager
                    .scalar_cell_observation(id.0, position.x, position.z);
                if changed {
                    object.handle_partition_cell_maintenance_with_partition_owner(
                        &mut self.partition_manager,
                    );
                    self.partition_manager
                        .finish_scalar_cell_observation(id.0, cell);
                }
            }
            // End the cell-maintenance mutable loan before neighboring actors
            // are observed. The same installed birth remains the contact source.
            let object = self.objects.get(&id).expect("partition-maintained Object");
            if dirty.check_collisions && !object.is_kind_of(KindOf::Immobile) {
                for other in self.partition_manager.neighbor_ids_in_coi_order(id.0) {
                    let pair = (id.0.min(other), id.0.max(other));
                    if !seen.insert(pair) {
                        continue;
                    }
                    let Some(other_object) = self.objects.get(&ObjectId(other)) else {
                        continue;
                    };
                    if let (Some(first), Some(second)) = (
                        self.partition_manager.receiving_contact_object(object),
                        self.partition_manager
                            .receiving_contact_object(other_object),
                    ) {
                        contacts.push(crate::game_logic::partition_manager::PartitionContact {
                            first,
                            second,
                        });
                    }
                }
            }
        }
        // PartitionContactList::addToContactList prepends each first pair.
        contacts.reverse();
        let mut handled = 0u32;
        for contact in contacts {
            let a_id = ObjectId(contact.first.id);
            let b_id = ObjectId(contact.second.id);
            let current = |owner: &Self| {
                owner.objects.get(&a_id).is_some_and(|object| {
                    owner
                        .partition_manager
                        .contact_object_is_current(&contact.first, object)
                }) && owner.objects.get(&b_id).is_some_and(|object| {
                    owner
                        .partition_manager
                        .contact_object_is_current(&contact.second, object)
                })
            };
            if !current(self) {
                continue;
            }
            let a = &self.objects[&a_id];
            let b = &self.objects[&b_id];
            if a.status.no_collisions || b.status.no_collisions {
                continue;
            }
            if super::collide_dispatch::host_geom_collides(a, b).is_none() {
                continue;
            }
            let a_radius = a.physics_on_collide_radius();
            let b_radius = b.physics_on_collide_radius();
            // Main actors dispatch receiving modules in try_physics_collide.
            // Core OBJECT_REGISTRY is a different world and must not be probed.
            if self.try_physics_collide_with_effects(a_id, b_id, a_radius, effects) {
                handled = handled.saturating_add(1);
            }
            let reverse = self
                .objects
                .get(&a_id)
                .is_some_and(|object| !object.status.destroyed)
                && self
                    .objects
                    .get(&b_id)
                    .is_some_and(|object| !object.status.destroyed);
            if reverse && self.try_physics_collide_with_effects(b_id, a_id, b_radius, effects) {
                handled = handled.saturating_add(1);
            }
            // Source rereads partition attachment after both onCollide calls.
            // Contact-caused geometry replacement may have a NEW module.
            for id in [a_id, b_id] {
                if let Some(object) = self.objects.get(&id) {
                    if !object.status.destroyed && !object.is_kind_of(KindOf::Immobile) {
                        self.partition_manager.mark_receiving_object_dirty(
                            id.0,
                            &object.visual_draw_identity_token,
                            false,
                        );
                    }
                }
            }
        }
        // Preserve the existing overlap residual recipients independently of
        // the original partition dirty/contact list. This is not query order.
        let enclosing_hidden = enclosing_hidden_rider_ids(self.objects.read());
        let mobile_ids: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(id, object)| {
                if object.drawable_hidden
                    || object.hijacker_in_vehicle
                    || enclosing_hidden.contains(&id.0)
                {
                    return None;
                }
                let wreck_physics = (object.status.destroyed || !object.is_alive())
                    && (object.movement.velocity.length_squared() > 1e-6
                        || object.get_position().y > object.ground_height
                        || object.allow_to_fall
                        || object.shock_stun_frames > 0);
                (object.can_move() || wreck_physics).then_some(*id)
            })
            .collect();
        for id in mobile_ids {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        // Stun/bounce residual on non-mobile still advances overlap bookkeeping.
        let extra: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.destroyed
                    && !o.can_move()
                    && (o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in extra {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        for o in self.objects.registered_values_mut() {
            o.landing_splat_done = false;
        }
        handled
    }

    pub(crate) fn tick_shock_stun_all(&mut self) {
        self.tick_shock_stun_all_with_effects(&mut Default::default())
            .expect("receiving stun callback failed")
    }

    pub(crate) fn tick_shock_stun_all_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Include bounce_audio_pending so land audio drains even after stun ends.
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            .map(|(id, _)| *id)
            .collect();
        let mut bounce_audio: Vec<(ObjectId, String, glam::Vec3, f32)> = Vec::new();
        for id in ids {
            let pos = self
                .objects
                .lookup(&id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            let (cliff, water) = self.sample_stun_surface_at(pos);
            let terrain_y = self
                .terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z))
                .unwrap_or(0.0);
            let ground_y = self
                .objects
                .lookup(&id)
                .map(|o| self.physics_ground_y_with_deck(o, terrain_y))
                .unwrap_or(terrain_y);
            if let Some(o) = self.objects.get_mut(&id) {
                o.cell_is_cliff = cliff;
                o.cell_is_underwater = water;
                o.ground_height = ground_y;
            }
            if self
                .objects
                .lookup(&id)
                .is_some_and(|o| o.shock_stun_frames > 0)
            {
                self.drive_owned_contact_with_effects(
                    id,
                    crate::game_logic::object::StunContact::Start,
                    Object::step_stun_contact_with_effects,
                    effects,
                )?;
            }
            if let Some(o) = self.objects.get_mut(&id) {
                while let Some((name, vol)) = o.take_bounce_audio_pending() {
                    bounce_audio.push((id, name, o.get_position(), vol));
                }
            }
        }
        // C++ TheAudio->addAudioEvent bounce residual.
        for (id, name, pos, vol) in bounce_audio {
            let pri = (64.0 + vol * 136.0).clamp(0.0, 255.0) as u8;
            self.queue_audio_event(
                AudioEventRequest::new(&name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(pri),
            );
        }
        Ok(())
    }

    pub(crate) fn apply_shock_wave_at_impact(
        &mut self,
        impact: glam::Vec3,
        source_pos: glam::Vec3,
        search_radius: f32,
        weapon_name: Option<&str>,
        skip_id: Option<ObjectId>,
    ) -> u32 {
        // Content cannot be changed by an earlier victim's callbacks. Keep
        // this accepted rule alive while preserving original visitation order.
        let native_rule = weapon_name.and_then(|name| {
            self.weapon_definitions
                .as_ref()?
                .get(&name.to_ascii_lowercase())
                .cloned()
        });
        if self.weapon_definitions.is_some() && weapon_name.is_some() && native_rule.is_none() {
            log::error!("Receiving weapon definition missing for accepted splash/shock");
            return 0;
        }

        use crate::game_logic::weapon_bootstrap::{
            compute_shock_wave_force, host_shock_wave_amount_for_weapon_name,
            host_shock_wave_radius_for_weapon_name, host_shock_wave_taper_for_weapon_name,
        };
        let Some(name) = weapon_name else {
            return 0;
        };
        let (amount, radius, taper) = native_rule
            .as_ref()
            .map(|rule| {
                (
                    rule.shock_wave_amount,
                    rule.shock_wave_radius,
                    rule.shock_wave_taper_off,
                )
            })
            .unwrap_or_else(|| {
                (
                    host_shock_wave_amount_for_weapon_name(name),
                    host_shock_wave_radius_for_weapon_name(name),
                    host_shock_wave_taper_for_weapon_name(name),
                )
            });
        if amount <= 0.0 || radius <= 0.0 || search_radius <= 0.0 {
            return 0;
        }
        let r2 = search_radius * search_radius;
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if skip_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                if dx * dx + dz * dz > r2 {
                    return None;
                }
                Some(*id)
            })
            .collect();
        let mut n = 0u32;
        for id in ids {
            let pos = match self.objects.lookup(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let Some(force) = compute_shock_wave_force(source_pos, pos, amount, radius, taper)
            else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&id) {
                if o.apply_shock_wave_impulse_with_operation(
                    force,
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
                ) {
                    n = n.saturating_add(1);
                }
            }
        }
        n
    }

    pub(crate) fn apply_instant_hit_splash_at(
        &mut self,
        impact: glam::Vec3,
        primary_damage: f32,
        secondary_damage: f32,
        primary_radius: f32,
        secondary_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        intended_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        self.apply_instant_hit_splash_at_with_effects(
            impact,
            primary_damage,
            secondary_damage,
            primary_radius,
            secondary_radius,
            attacker_id,
            attacker_team,
            intended_id,
            weapon_name,
            &mut Default::default(),
        )
    }

    pub(crate) fn apply_instant_hit_splash_at_with_effects(
        &mut self,
        impact: glam::Vec3,
        primary_damage: f32,
        secondary_damage: f32,
        primary_radius: f32,
        secondary_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        intended_id: ObjectId,
        weapon_name: Option<&str>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        // Content cannot be changed by an earlier victim's callbacks. Keep
        // this accepted rule alive while preserving original visitation order.
        let native_rule = weapon_name.and_then(|name| {
            self.weapon_definitions
                .as_ref()?
                .get(&name.to_ascii_lowercase())
                .cloned()
        });
        if self.weapon_definitions.is_some() && weapon_name.is_some() && native_rule.is_none() {
            log::error!("Receiving weapon definition missing for accepted splash/shock");
            return 0;
        }

        let max_r = primary_radius.max(secondary_radius);
        if max_r <= 0.0 || (primary_damage <= 0.0 && secondary_damage <= 0.0) {
            return 0;
        }
        use crate::game_logic::weapon_bootstrap::{
            WEAPON_AFFECTS_DEFAULT, host_radius_damage_affects_for_weapon_name,
            radius_damage_affects_victim,
        };
        let affects = native_rule
            .as_ref()
            .map(|rule| rule.affects_mask.bits())
            .unwrap_or_else(|| {
                weapon_name
                    .map(host_radius_damage_affects_for_weapon_name)
                    .unwrap_or(WEAPON_AFFECTS_DEFAULT)
            });
        let (
            shooter_template,
            attacker_owner,
            attacker_team_id,
            attacker_team_instance,
            attacker_producer,
            attacker_is_defector,
        ) = self
            .objects
            .lookup(&attacker_id)
            .map(|a| {
                (
                    a.template_name.clone(),
                    a.owner_player_id,
                    a.team_instance_id,
                    a.team_instance_name.clone(),
                    a.producer_id,
                    a.is_undetected_defector(),
                )
            })
            .unwrap_or_default();
        let source_present = self.objects.lookup(&attacker_id).is_some();
        // C++ Weapon.cpp dealDamageInternal iterates partition-world objects
        // only. Enclosing occupants (tunnel/transport) unRegisterObject on
        // enter, so splash at an entrance must not hit the shared pool.
        let enclosing_hidden = enclosing_hidden_rider_ids(self.objects.read());
        let tunnel_held: std::collections::HashSet<u32> = self
            .tunnel_network
            .occupant_player_ids()
            .into_iter()
            .flat_map(|pid| self.tunnel_network.contained_for_player(pid))
            .map(|id| id.0)
            .collect();
        let players = &self.players;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if *id == intended_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                if enclosing_hidden.contains(&id.0) || tunnel_held.contains(&id.0) {
                    return None;
                }
                let airborne = obj.is_significantly_above_terrain();
                if source_present {
                    let same_tmpl = crate::game_logic::weapon_bootstrap::receiving_splash_templates_equivalent(
                        &self.templates,
                        &shooter_template,
                        &obj.template_name,
                    );
                    let (similar_relationship, mask_relationship) =
                        receiving_shock_splash_relationships(
                            &self.team_factory,
                            players,
                            attacker_owner,
                            attacker_team_id,
                            &attacker_team_instance,
                            attacker_is_defector,
                            obj,
                        );
                    let doesnt_affect_similar =
                        crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_DOESNT_AFFECT_SIMILAR;
                    if (affects & doesnt_affect_similar) != 0
                        && same_tmpl
                        && similar_relationship == gamelogic::common::Relationship::Allies
                    {
                        return None;
                    }
                    if !radius_damage_affects_victim(
                        affects & !doesnt_affect_similar,
                        mask_relationship,
                        attacker_id,
                        *id,
                        attacker_producer,
                        airborne,
                        false,
                    ) {
                        return None;
                    }
                }
                let dist = crate::game_logic::combat::splash_from_bounding_sphere_3d(
                    impact,
                    obj.get_position(),
                    crate::game_logic::combat::victim_splash_sphere_radius(obj),
                );
                if dist > max_r {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();
        let mut hits = 0u32;
        for (id, dist) in candidates {
            let dmg = if primary_radius > 0.0 && dist <= primary_radius {
                primary_damage
            } else if secondary_damage > 0.0 && dist <= max_r {
                secondary_damage
            } else {
                0.0
            };
            if dmg <= 0.0 {
                continue;
            }
            // Earlier victims may synchronously kill another snapshotted
            // candidate through onDie. Re-fetch instead of replaying that hit.
            if self.objects.lookup(&id).is_some_and(|obj| obj.is_alive()) {
                let damage_type = native_rule
                    .as_ref()
                    .map(|rule| {
                        crate::game_logic::host_armor_residual::map_store_damage_type(
                            rule.damage_type,
                        )
                    })
                    .unwrap_or_else(|| {
                        weapon_name
                    .map(crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name)
                    .unwrap_or(crate::game_logic::combat::DamageType::Explosive)
                    });
                let death_type = native_rule
                    .as_ref()
                    .map(|rule| {
                        crate::game_logic::host_usa_pilot::HostDeathType::from_store(
                            rule.death_type,
                        )
                    })
                    .unwrap_or_else(|| {
                        crate::game_logic::host_armor_residual::resolve_host_death_type(
                            weapon_name,
                            damage_type,
                        )
                    });
                let context = crate::game_logic::object::DamageHitContext::new_selected(
                    self.objects.lookup(&attacker_id),
                    weapon_name,
                    damage_type,
                    native_rule.as_deref(),
                );
                // C++ Weapon::dealDamageInternal completes each attemptDamage:
                // body callbacks -> scoreTheKill -> onDie -> fresh-source FX.
                if self
                    .apply_owned_damage_with_killer_team_with_effects(
                        id,
                        dmg,
                        Some(attacker_id),
                        damage_type,
                        death_type,
                        None,
                        &context,
                        Some(attacker_team),
                        effects,
                    )
                    .is_some()
                {
                    hits = hits.saturating_add(1);
                }
            }
        }
        let source_pos = self
            .objects
            .lookup(&attacker_id)
            .map(|a| a.get_position())
            .unwrap_or(impact);
        let _ = self.apply_shock_wave_at_impact(impact, source_pos, max_r, weapon_name, None);
        hits
    }

    pub(crate) fn apply_scatter_miss_splash_at(
        &mut self,
        impact: glam::Vec3,
        weapon_damage: f32,
        splash_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        skip_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        self.apply_scatter_miss_splash_at_with_effects(
            impact,
            weapon_damage,
            splash_radius,
            attacker_id,
            attacker_team,
            skip_id,
            weapon_name,
            &mut Default::default(),
        )
    }

    pub(crate) fn apply_scatter_miss_splash_at_with_effects(
        &mut self,
        impact: glam::Vec3,
        weapon_damage: f32,
        splash_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        skip_id: ObjectId,
        weapon_name: Option<&str>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        // Content cannot be changed by an earlier victim's callbacks. Keep
        // this accepted rule alive while preserving original visitation order.
        let native_rule = weapon_name.and_then(|name| {
            self.weapon_definitions
                .as_ref()?
                .get(&name.to_ascii_lowercase())
                .cloned()
        });
        if self.weapon_definitions.is_some() && weapon_name.is_some() && native_rule.is_none() {
            log::error!("Receiving weapon definition missing for accepted splash/shock");
            return 0;
        }

        use crate::game_logic::weapon_bootstrap::{
            WEAPON_AFFECTS_DEFAULT, host_primary_damage_radius_for_weapon_name,
            host_radius_damage_affects_for_weapon_name, host_secondary_damage_for_weapon_name,
            host_secondary_damage_radius_for_weapon_name, radius_damage_affects_victim,
        };
        // C++ dealDamageInternal: authored primary/secondary radii, no 1.5x ring.
        let (damage_mult, radius_mult) = self.objects.lookup(&attacker_id).map_or(
            (1.0, 1.0),
            |source| {
                let bonus = if weapon_name.is_some() {
                    self.receiving_weapon_bonus_fields(source, native_rule.as_deref())
                } else {
                    self.receiving_weapon_bonus_fields_for_slot(source, source.active_weapon_slot)
                };
                (bonus.0, bonus.4)
            },
        );
        let (primary_r, secondary_r, primary_dmg, secondary_dmg) =
            if let Some(rule) = native_rule.as_ref() {
                (
                    rule.primary_damage_radius * radius_mult,
                    rule.secondary_damage_radius * radius_mult,
                    weapon_damage,
                    rule.secondary_damage * damage_mult,
                )
            } else {
                match weapon_name {
                    Some(n) => {
                        let pr = host_primary_damage_radius_for_weapon_name(n) * radius_mult;
                        let sr = host_secondary_damage_radius_for_weapon_name(n) * radius_mult;
                        let sd = host_secondary_damage_for_weapon_name(n) * damage_mult;
                        let primary = if pr > 0.0 {
                            pr
                        } else {
                            splash_radius * radius_mult
                        };
                        (primary, sr, weapon_damage, sd)
                    }
                    None => (splash_radius, 0.0, weapon_damage, 0.0),
                }
            };
        let max_r = primary_r.max(secondary_r);
        if max_r <= 0.0 || (primary_dmg <= 0.0 && secondary_dmg <= 0.0) {
            return 0;
        }
        let affects = native_rule
            .as_ref()
            .map(|rule| rule.affects_mask.bits())
            .unwrap_or_else(|| {
                weapon_name
                    .map(host_radius_damage_affects_for_weapon_name)
                    .unwrap_or(WEAPON_AFFECTS_DEFAULT)
            });
        let (
            shooter_template,
            attacker_owner,
            attacker_team_id,
            attacker_team_instance,
            attacker_producer,
            attacker_is_defector,
        ) = self
            .objects
            .lookup(&attacker_id)
            .map(|a| {
                (
                    a.template_name.clone(),
                    a.owner_player_id,
                    a.team_instance_id,
                    a.team_instance_name.clone(),
                    a.producer_id,
                    a.is_undetected_defector(),
                )
            })
            .unwrap_or_default();
        let source_present = self.objects.lookup(&attacker_id).is_some();
        let players = &self.players;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if *id == skip_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                let airborne = obj.is_significantly_above_terrain();
                if source_present {
                    let same_tmpl = crate::game_logic::weapon_bootstrap::receiving_splash_templates_equivalent(
                        &self.templates,
                        &shooter_template,
                        &obj.template_name,
                    );
                    let (mut similar_relationship, mut mask_relationship) =
                        receiving_shock_splash_relationships(
                            &self.team_factory,
                            players,
                            attacker_owner,
                            attacker_team_id,
                            &attacker_team_instance,
                            attacker_is_defector,
                            obj,
                        );
                    // C++ Team-only fallback when neither object has an owner.
                    if attacker_owner.is_none()
                        && obj.owner_player_id.is_none()
                        && mask_relationship == gamelogic::common::Relationship::Neutral
                    {
                        mask_relationship = if obj.team == attacker_team {
                            gamelogic::common::Relationship::Allies
                        } else if obj.team != Team::Neutral {
                            gamelogic::common::Relationship::Enemies
                        } else {
                            gamelogic::common::Relationship::Neutral
                        };
                    }
                    if attacker_owner.is_none()
                        && obj.owner_player_id.is_none()
                        && similar_relationship == gamelogic::common::Relationship::Neutral
                    {
                        similar_relationship = if attacker_team == obj.team {
                            gamelogic::common::Relationship::Allies
                        } else if attacker_team != Team::Neutral && obj.team != Team::Neutral {
                            gamelogic::common::Relationship::Enemies
                        } else {
                            gamelogic::common::Relationship::Neutral
                        };
                    }
                    let doesnt_affect_similar =
                        crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_DOESNT_AFFECT_SIMILAR;
                    if (affects & doesnt_affect_similar) != 0
                        && same_tmpl
                        && similar_relationship == gamelogic::common::Relationship::Allies
                    {
                        return None;
                    }
                    if !radius_damage_affects_victim(
                        affects & !doesnt_affect_similar,
                        mask_relationship,
                        attacker_id,
                        *id,
                        attacker_producer,
                        airborne,
                        false,
                    ) {
                        return None;
                    }
                }
                let dist = crate::game_logic::combat::splash_from_bounding_sphere_3d(
                    impact,
                    obj.get_position(),
                    crate::game_logic::combat::victim_splash_sphere_radius(obj),
                );
                if dist > max_r {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();
        let mut hits = 0u32;
        for (id, dist) in candidates {
            let dmg = if dist <= primary_r {
                primary_dmg
            } else {
                secondary_dmg
            };
            if dmg <= 0.0 {
                continue;
            }
            // Earlier victims may synchronously kill another snapshotted
            // candidate through onDie. Re-fetch instead of replaying that hit.
            if self.objects.lookup(&id).is_some_and(|obj| obj.is_alive()) {
                let damage_type = native_rule
                    .as_ref()
                    .map(|rule| {
                        crate::game_logic::host_armor_residual::map_store_damage_type(
                            rule.damage_type,
                        )
                    })
                    .unwrap_or_else(|| {
                        weapon_name
                    .map(crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name)
                    .unwrap_or(crate::game_logic::combat::DamageType::Explosive)
                    });
                let death_type = native_rule
                    .as_ref()
                    .map(|rule| {
                        crate::game_logic::host_usa_pilot::HostDeathType::from_store(
                            rule.death_type,
                        )
                    })
                    .unwrap_or_else(|| {
                        crate::game_logic::host_armor_residual::resolve_host_death_type(
                            weapon_name,
                            damage_type,
                        )
                    });
                let context = crate::game_logic::object::DamageHitContext::new_selected(
                    self.objects.lookup(&attacker_id),
                    weapon_name,
                    damage_type,
                    native_rule.as_deref(),
                );
                // C++ Weapon::dealDamageInternal completes each attemptDamage:
                // body callbacks -> scoreTheKill -> onDie -> fresh-source FX.
                if self
                    .apply_owned_damage_with_killer_team_with_effects(
                        id,
                        dmg,
                        Some(attacker_id),
                        damage_type,
                        death_type,
                        None,
                        &context,
                        Some(attacker_team),
                        effects,
                    )
                    .is_some()
                {
                    hits = hits.saturating_add(1);
                }
            }
        }
        let source_pos = self
            .objects
            .lookup(&attacker_id)
            .map(|a| a.get_position())
            .unwrap_or(impact);
        let _ =
            self.apply_shock_wave_at_impact(impact, source_pos, max_r, weapon_name, Some(skip_id));
        hits
    }

    pub(crate) fn instant_scatter_misses_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            DEFAULT_SCATTER_HIT_RADIUS, host_effective_scatter_radius,
            scatter_misses_intended_target, scatter_seed_for_shot,
        };
        let (wname, tgt_inf, hit_r) = {
            let attacker = match self.objects.lookup(&attacker_id) {
                Some(a) => a,
                None => return false,
            };
            let target = match self.objects.lookup(&target_id) {
                Some(t) => t,
                None => return false,
            };
            let wname = attacker.weapon_name_for_slot(slot);
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            (
                wname.map(|s| s.to_string()),
                target.is_kind_of(KindOf::Infantry),
                hit_r,
            )
        };
        let Some(name) = wname else {
            return false;
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        if scatter <= 0.0 {
            return false;
        }
        let seed = scatter_seed_for_shot(attacker_id.0, target_id.0, self.frame);
        scatter_misses_intended_target(scatter, seed, hit_r)
    }

    pub(crate) fn try_min_range_backup(
        &mut self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        min_range: f32,
    ) -> bool {
        self.try_min_range_backup_between(attacker_id, target_pos, min_range, 0.0, None)
    }

    /// C++ `Weapon.cpp:2043`: stand at `(getAttackRange + getMinimumAttackRange) / 2`
    /// when `max_range > min_range`. Otherwise keep the 110% residual.
    pub(crate) fn try_min_range_backup_between(
        &mut self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        min_range: f32,
        max_range: f32,
        target_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::is_inside_minimum_attack_range;
        let Some(attacker) = self.objects.lookup(&attacker_id) else {
            return false;
        };
        if !attacker.can_move() || !attacker.is_alive() {
            return false;
        }
        let src = attacker.get_position();
        let orientation = attacker.get_orientation();
        let above = attacker.is_above_terrain();
        let src_r = (*attacker.geometry_info()).bounding_circle_radius();
        let mut dx = src.x - target_pos.x;
        let mut dz = src.z - target_pos.z;
        let center = (dx * dx + dz * dz).sqrt();
        let tgt_r = target_id
            .and_then(|id| self.objects.lookup(&id))
            .map(|o| (*o.geometry_info()).bounding_circle_radius())
            .unwrap_or(0.0);
        // C++ getVectorTo FROM_BOUNDINGSPHERE_2D.
        let dist = (center - src_r - tgt_r).max(0.0);
        if !is_inside_minimum_attack_range(dist, min_range) {
            return false;
        }
        // C++ Weapon.cpp:2020-2032. Above terrain, do not U-turn if the
        // through-target heading is within 90 degrees of facing.
        if above {
            let angle = (-dz).atan2(-dx);
            let mut rel = orientation - angle;
            let tau = std::f32::consts::TAU;
            if rel > tau {
                rel -= tau;
            }
            if rel < -tau {
                rel += tau;
            }
            if rel.abs() < std::f32::consts::FRAC_PI_2 {
                dx = -dx;
                dz = -dz;
            }
        }
        let dest = if max_range > min_range {
            let stand = (min_range + max_range) * 0.5 + src_r + tgt_r;
            if center < 1e-3 {
                glam::Vec3::new(target_pos.x + stand, src.y, target_pos.z)
            } else {
                let scale = stand / center;
                glam::Vec3::new(target_pos.x + dx * scale, src.y, target_pos.z + dz * scale)
            }
        } else {
            let stand =
                crate::game_logic::weapon_bootstrap::effective_minimum_attack_range(min_range);
            if stand <= 0.0 || center < 1e-3 {
                glam::Vec3::new(target_pos.x + stand, src.y, target_pos.z)
            } else {
                let scale = stand / center;
                glam::Vec3::new(target_pos.x + dx * scale, src.y, target_pos.z + dz * scale)
            }
        };
        // C++ clipToTerrainExtent: one full cell inside the terrain X/ground extent.
        // Host ground plane is XZ. Height is left alone.
        let (lo_x, lo_z, hi_x, hi_z) = self.ocl_map_extents();
        let cell = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE;
        let lo_x = lo_x + cell;
        let hi_x = hi_x - cell;
        let lo_z = lo_z + cell;
        let hi_z = hi_z - cell;
        let mut dest = dest;
        if hi_x > lo_x {
            dest.x = dest.x.clamp(lo_x, hi_x);
        }
        if hi_z > lo_z {
            dest.z = dest.z.clamp(lo_z, hi_z);
        }
        // Direct backup residual (fail-closed vs full reverse-pathfind matrix).
        if let Some(a) = self.objects.get_mut(&attacker_id) {
            a.movement.path.clear();
            a.movement.current_path_index = 0;

            a.movement.target_position = Some(dest);
            let entered_attack = !matches!(
                a.ai_state(),
                AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
            );
            if entered_attack {
                a.set_ai_state(
                    AIState::Attacking,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            a.set_status_attacking(true);
            {}
            a.set_status_moving(true);
            self.movement_events
                .record(self.frame, attacker_id, Some([dest.x, dest.y, dest.z]));
            return true;
        }
        false
    }

    pub(crate) fn approach_pos_for_attack(
        &self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        weapon_range: f32,
        weapon_name: Option<&str>,
        target_id: Option<ObjectId>,
    ) -> glam::Vec3 {
        let Some(attacker) = self.objects.lookup(&attacker_id) else {
            return target_pos;
        };
        let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
        let raw_gap = attacker
            .selected_weapon_slot()
            .and_then(|slot| attacker.weapon_slot(slot))
            .map(|w| w.range - under)
            .unwrap_or(weapon_range);
        let src = attacker.get_position();
        let src_r = (*attacker.geometry_info()).bounding_circle_radius();
        let tgt_r = target_id
            .and_then(|id| self.objects.lookup(&id))
            .map(|o| (*o.geometry_info()).bounding_circle_radius())
            .unwrap_or(0.0);
        let dx = target_pos.x - src.x;
        let dz = target_pos.z - src.z;
        let gap = ((dx * dx + dz * dz).sqrt() - src_r - tgt_r).max(0.0);
        if gap < 0.001 {
            return src;
        }
        let contact = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
            .unwrap_or(false)
            || crate::game_logic::weapon_bootstrap::is_contact_effective_range(raw_gap);
        if contact {
            return target_pos;
        }
        let dest = crate::game_logic::weapon_bootstrap::compute_approach_target_pos_at(
            src,
            target_pos,
            weapon_range,
            false,
        );

        self.adjust_aircraft_attack_approach(attacker_id, dest, target_pos, weapon_range, 0.0)
    }

    /// C++ `isAircraftThatAdjustsDestination` + leftover `adjustTargetDestination`
    /// after range*0.9 so HOVER/WINGS do not stack on one hover cell.
    pub(crate) fn adjust_aircraft_attack_approach(
        &self,
        attacker_id: ObjectId,
        dest: glam::Vec3,
        target_pos: glam::Vec3,
        weapon_range: f32,
        min_range: f32,
    ) -> glam::Vec3 {
        let Some(obj) = self.objects.lookup(&attacker_id) else {
            return dest;
        };
        if !PathfindingGrid::is_aircraft_that_adjusts_destination(obj) {
            return dest;
        }
        let src_r = (*obj.geometry_info()).bounding_circle_radius();
        let unit_radius = obj.selection_radius.max(src_r);
        let tgt_r = self
            .objects
            .registered_values()
            .filter(|o| o.id != attacker_id && o.is_alive())
            .find(|o| {
                let p = o.get_position();
                let dx = p.x - target_pos.x;
                let dz = p.z - target_pos.z;
                dx * dx + dz * dz < 1.0
            })
            .map(|o| (*o.geometry_info()).bounding_circle_radius())
            .unwrap_or(0.0);
        let min_range = self
            .objects
            .lookup(&attacker_id)
            .and_then(|a| {
                a.selected_weapon_slot()
                    .and_then(|s| a.weapon_slot(s))
                    .map(|w| w.min_range)
            })
            .unwrap_or(min_range);
        let airborne = obj.status.airborne_target
            || obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft;
        let surfaces = if airborne {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        self.pathfinding_system.adjust_target_destination(
            attacker_id.0,
            self.objects.read(),
            dest,
            target_pos,
            unit_radius,
            surfaces,
            obj.crusher_level > 0,
            src_r,
            tgt_r,
            weapon_range,
            min_range,
        )
    }

    pub(crate) fn try_continue_attack_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        if continue_range <= 0.0 {
            return false;
        }
        let can_air = self
            .objects
            .lookup(&attacker_id)
            .and_then(|attacker| {
                attacker.selected_weapon_slot().and_then(|slot| {
                    attacker
                        .weapon_slot(slot)
                        .map(|weapon| weapon.can_target_air)
                })
            })
            .unwrap_or(false);
        let attacker_off_map = self
            .objects
            .lookup(&attacker_id)
            .map(|attacker| {
                crate::game_logic::host_deliver_payload::is_off_map_default_residual(
                    attacker.get_position(),
                )
            })
            .unwrap_or(false);
        let victim_owner = self
            .objects
            .lookup(&dead_victim_id)
            .and_then(|victim| victim.owner_player_id);
        // Pure residual acquire: nearest same-team victim near kill position (XZ).
        let candidates: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(&id, obj)| {
                if id == attacker_id || id == dead_victim_id || !obj.is_alive() {
                    return None;
                }
                let same_player = if let Some(owner) = victim_owner {
                    obj.owner_player_id == Some(owner)
                } else {
                    obj.team == victim_team
                };
                if !same_player
                    || obj.is_effectively_stealthed()
                    || obj.status.under_construction
                    || obj.is_eject_invulnerable()
                    || obj.contained_by.is_some()
                    || (!obj.is_attackable() && !obj.is_disarmable_mine())
                    || crate::game_logic::host_deliver_payload::is_off_map_default_residual(
                        obj.get_position(),
                    ) != attacker_off_map
                    || ((obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target)
                        && !can_air)
                {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: obj.is_effectively_stealthed(),
                        is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                        eject_invulnerable: obj.is_eject_invulnerable(),
                    },
                )
            })
            .collect();
        let Some((next_id, _, _)) =
            crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(attacker_id),
                (original_victim_pos.x, original_victim_pos.z),
                candidates,
                continue_range,
                |_| true,
            )
        else {
            return false;
        };
        {}
        let next_pos = self.objects.lookup(&next_id).map(|obj| obj.get_position());
        let armed = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            attacker.target = Some(next_id);
            attacker.set_ai_state(
                AIState::Attacking,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            attacker.set_status_attacking(true);
            true
        } else {
            false
        };
        if armed {
            if let Some(pos) = next_pos {
                let _ = self.assign_unit_attack_path(attacker_id, Some(next_id), pos);
            }
        }
        armed
    }

    #[cfg(test)]
    pub fn try_continue_attack_after_kill_for_test(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            continue_range,
            victim_team,
        )
    }

    /// C++ ExperienceTracker::addExperiencePoints sink + trainable gate.
    pub fn award_experience(&mut self, recipient_id: ObjectId, amount: f32) {
        self.award_experience_with_effects_and_interpreter(
            recipient_id,
            amount,
            None,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("standalone experience award must complete");
    }

    pub(crate) fn award_experience_with_effects_and_interpreter(
        &mut self,
        recipient_id: ObjectId,
        amount: f32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if amount <= 0.0 || !amount.is_finite() {
            return Ok(());
        }
        let Some(obj) = self.objects.lookup(&recipient_id) else {
            return Ok(());
        };
        if !obj.is_accepting_experience_points() {
            return Ok(());
        }
        // C++ forwards `experienceGain * m_experienceScalar` to the sink, then
        // the sink may scale again. Untrainable objects without an explicit
        // sink do not accept XP (no implicit producer_id).
        let source_scalar = if obj.experience_scalar.is_finite() && obj.experience_scalar > 0.0 {
            obj.experience_scalar
        } else {
            1.0
        };
        let dest = obj
            .experience_sink
            .filter(|sid| *sid != recipient_id && self.objects.contains_key(sid));
        let (dest_id, forwarded) = if let Some(sink_id) = dest {
            (sink_id, amount * source_scalar)
        } else {
            (recipient_id, amount)
        };
        let transition = self
            .objects
            .get_mut(&dest_id)
            .and_then(|object| object.commit_experience_gain_for_world(forwarded));
        if let Some((previous, level)) = transition {
            self.finish_experience_rank_change_with_upgrade_callback(
                dest_id,
                previous,
                level,
                interpreter,
                effects,
            )?;
        }
        Ok(())
    }

    /// C++ Object::scoreTheKill unit XP for residual apply / capture paths.
    pub(crate) fn award_score_the_kill_experience(
        &mut self,
        killer_id: ObjectId,
        victim_id: ObjectId,
    ) {
        self.award_score_the_kill_experience_with_effects_and_interpreter(
            killer_id,
            victim_id,
            None,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("compatibility kill XP award must complete");
    }

    pub(crate) fn award_score_the_kill_experience_with_effects_and_interpreter(
        &mut self,
        killer_id: ObjectId,
        victim_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(victim) = self.objects.lookup(&victim_id) else {
            return Ok(());
        };
        if victim.is_alive() && victim.health.current > 0.0 && !victim.status.destroyed {
            return Ok(());
        }
        self.score_the_kill_experience_with_effects_and_interpreter(
            killer_id,
            victim_id,
            interpreter,
            effects,
        )
    }

    /// Explicit CPP Object::scoreTheKill may precede Body death, as in
    /// AIRappelState. Ordinary combat admission keeps its dead-victim gate.
    pub(crate) fn score_the_kill_experience(&mut self, killer_id: ObjectId, victim_id: ObjectId) {
        self.score_the_kill_experience_with_effects_and_interpreter(
            killer_id,
            victim_id,
            None,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("compatibility score-the-kill XP must complete");
    }

    fn score_the_kill_experience_with_effects_and_interpreter(
        &mut self,
        killer_id: ObjectId,
        victim_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::kill_credit::KillScorePhase;
        let Some((victim_score_ready, victim_birth)) =
            self.objects.lookup(&victim_id).map(|victim| {
                (
                    victim.kill_score_phase == KillScorePhase::Ready,
                    std::sync::Arc::downgrade(&victim.visual_draw_identity_token),
                )
            })
        else {
            return Ok(());
        };
        if !victim_score_ready {
            return Ok(());
        }
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            victim.kill_score_phase = KillScorePhase::Running;
        }
        self.record_owned_score_the_kill(killer_id, victim_id);
        // Preserve the independently persisted historical XP receipt. It must
        // never be reinterpreted as proof that full Player scoring occurred.
        let award = self
            .objects
            .lookup(&victim_id)
            .filter(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            })
            .and_then(|victim| {
                (!victim.kill_experience_awarded)
                    .then_some(victim.kill_experience_value())
            });
        if let Some(xp) = award {
            if let Some(victim) = self.objects.get_mut(&victim_id).filter(|victim| {
                victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &victim.visual_draw_identity_token,
                ))
            }) {
                victim.kill_experience_awarded = true;
            }
            if self.kill_awards_unit_experience(killer_id, victim_id) {
                self.award_experience_with_effects_and_interpreter(
                    killer_id,
                    xp,
                    interpreter,
                    effects,
                )?;
            }
        }
        if let Some(victim) = self.objects.get_mut(&victim_id).filter(|victim| {
            victim_birth.ptr_eq(&std::sync::Arc::downgrade(
                &victim.visual_draw_identity_token,
            ))
        }) {
            victim.kill_score_phase = KillScorePhase::Complete;
        }
        Ok(())
    }

    /// C++ Object::scoreTheKill + getExperienceValue: only ENEMIES, not own/allies.
    /// Also skips non-playable sides and KINDOF_IGNORED_IN_GUI (Object.cpp:2898-2905).
    fn kill_awards_unit_experience(
        &self,
        killer_id: ObjectId,
        victim_id: ObjectId,
    ) -> bool {
        use gamelogic::common::Relationship;
        let Some(killer) = self.objects.lookup(&killer_id) else {
            return false;
        };
        let Some(victim) = self.objects.lookup(&victim_id) else {
            return false;
        };
        let killer_owner = self.player_owner_for_host_object(killer);
        let victim_owner = self.player_owner_for_host_object(victim);
        self.score_the_kill_victim_counts(victim)
            && !(killer_owner.is_some() && killer_owner == victim_owner)
            && self.object_relationship(killer, victim) == Relationship::Enemies
    }

    /// Effective radii from this session's immutable catalog. Missing entries
    /// in an admitted catalog are not supplied by another world's store.
    pub(crate) fn receiving_weapon_radii(
        &self,
        name: &str,
    ) -> crate::game_logic::weapon_bootstrap::ReceivingWeaponRadii {
        use crate::game_logic::weapon_bootstrap::ReceivingWeaponRadii;
        match self.weapon_definitions.as_ref() {
            Some(definitions) => definitions
                .get(&name.trim().to_ascii_lowercase())
                .map_or_else(ReceivingWeaponRadii::default, |weapon| {
                    ReceivingWeaponRadii::from_template(weapon)
                }),
            None => ReceivingWeaponRadii {
                primary_damage: crate::game_logic::weapon_bootstrap::seed_primary_damage_radius_for(
                    name,
                ),
                secondary_damage:
                    crate::game_logic::weapon_bootstrap::seed_secondary_damage_radius_for(name),
                scatter: crate::game_logic::weapon_bootstrap::seed_scatter_radius_for(name),
                scatter_infantry:
                    crate::game_logic::weapon_bootstrap::seed_scatter_radius_vs_infantry_for(name),
                scatter_target_scalar: 0.0,
            },
        }
    }

    /// Append this weapon's extra set after the receiving world's SOLO bonuses
    /// and existing condition handling, including container conditions.
    pub(in crate::game_logic) fn receiving_weapon_bonus_fields(
        &self,
        source: &Object,
        definition: Option<&gamelogic::weapon::WeaponTemplate>,
    ) -> (f32, f32, f32, f32, f32) {
        let container = source.contained_by.and_then(|id| self.host_object(id));
        source.weapon_bonus_fields_with_owned_extra(
            container,
            &self.solo_weapon_bonus_set,
            definition.and_then(|weapon| weapon.extra_bonus.as_ref()),
        )
    }

    pub(in crate::game_logic) fn receiving_weapon_bonus_fields_for_slot(
        &self,
        source: &Object,
        slot: u8,
    ) -> (f32, f32, f32, f32, f32) {
        let native = source.selected_native_weapon_rule(slot);
        let name = source
            .selected_weapon_name(slot)
            .or_else(|| source.authored_weapon_name_for_slot(slot));
        let definition = native.as_deref().or_else(|| {
            self.weapon_definitions.as_ref().and_then(|definitions| {
                name.and_then(|name| definitions.get(&name.trim().to_ascii_lowercase()))
                    .map(AsRef::as_ref)
            })
        });
        self.receiving_weapon_bonus_fields(source, definition)
    }

    pub(in crate::game_logic) fn receiving_weapon_rates_of_fire(
        &self,
        source: &Object,
    ) -> [f32; 3] {
        std::array::from_fn(|slot| {
            self.receiving_weapon_bonus_fields_for_slot(source, slot as u8).2
        })
    }

    /// C++ InGameUI::setRadiusCursor reads the requested WeaponSet slot from
    /// the selected object. The session owns both its definition and bonuses.
    pub fn weapon_cursor_radii(
        &self,
        id: ObjectId,
        slot: u8,
    ) -> Option<crate::game_logic::weapon_bootstrap::WeaponCursorRadii> {
        use crate::game_logic::weapon_bootstrap::{ReceivingWeaponRadii, WeaponCursorRadii};
        let object = self.host_object(id)?;
        let weapon = object.weapon_slot(slot)?;
        let name = object
            .selected_weapon_name(slot)
            .or_else(|| object.authored_weapon_name_for_slot(slot));
        let native = object.selected_native_weapon_rule(slot);
        let radii = native
            .as_deref()
            .map(ReceivingWeaponRadii::from_template)
            .or_else(|| name.map(|name| self.receiving_weapon_radii(name)))
            .unwrap_or(ReceivingWeaponRadii {
                primary_damage: weapon.splash_radius,
                ..Default::default()
            });
        let continue_attack = native.as_ref().map_or_else(
            || name.map_or(0.0, |name| self.receiving_continue_attack_range(name)),
            |rule| rule.continue_attack_range.max(0.0),
        );
        let multiplier = self.receiving_weapon_bonus_fields_for_slot(object, slot).4;
        Some(WeaponCursorRadii {
            damage: radii.primary_damage * multiplier,
            scatter: radii.scatter + radii.scatter_target_scalar,
            continue_attack,
        })
    }

    /// Resolve the effective definition in this session. An admitted zero is
    /// authoritative; seed data is only for sessions without a catalog.
    pub(crate) fn receiving_continue_attack_range(&self, name: &str) -> f32 {
        match self.weapon_definitions.as_ref() {
            Some(definitions) => definitions
                .get(&name.trim().to_ascii_lowercase())
                .map_or(0.0, |weapon| weapon.continue_attack_range.max(0.0)),
            None => crate::game_logic::weapon_bootstrap::seed_continue_attack_range_for(name),
        }
    }

    /// After a combat kill: grant XP, ContinueAttackRange retarget, else stop.
    pub(crate) fn continue_or_stop_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        victim_team: Team,
        weapon_name: Option<&str>,
    ) {
        // The owned damage executor scores before onDie/FX. Retain once-only
        // credit for residual callers that have not migrated to that executor.
        self.award_score_the_kill_experience(attacker_id, dead_victim_id);
        let cont = weapon_name.map_or(0.0, |name| self.receiving_continue_attack_range(name));
        if self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            cont,
            victim_team,
        ) {
            return;
        }
        self.stop_attack_decision_aware(attacker_id);
    }
}

/// C++ `OpenContain::addOrRemoveObjFromWorld` — enclosing riders leave the
/// collide partition on enter and re-register on exit. This helper only
/// observes containment for unrelated legacy query filters; it never rebuilds COIs.
fn enclosing_hidden_rider_ids(
    objects: crate::game_logic::HostObjectRead<'_>,
) -> std::collections::HashSet<u32> {
    let pairs: Vec<(ObjectId, ObjectId, String)> = objects
        .registered()
        .filter_map(|(id, o)| {
            o.contained_by
                .map(|cid| (*id, cid, o.template_name.clone()))
        })
        .collect();
    pairs
        .into_iter()
        .filter(|(rid, cid, tmpl)| {
            objects
                .lookup(cid)
                .is_some_and(|c| enclosing_container_hides_rider(c, *rid, tmpl))
        })
        .map(|(rid, _, _)| rid.0)
        .collect()
}

/// Mirrors `Object::is_enclosing_container_for` without dual HashMap borrows.
fn enclosing_container_hides_rider(
    container: &Object,
    victim_id: ObjectId,
    victim_template: &str,
) -> bool {
    let name = container.template_name.to_ascii_lowercase();
    if container.paradrop_parachute || name.contains("parachute") {
        return false;
    }
    if container.is_overlord_style_container() {
        if container.overlord_portable_occupant == Some(victim_id) {
            return false;
        }
        if container
            .contained_units()
            .first()
            .is_some_and(|&id| id == victim_id)
        {
            return false;
        }
        if crate::game_logic::host_battlemaster::is_portable_structure_template(victim_template) {
            return false;
        }
    }
    if container.is_helix_transport
        && crate::game_logic::host_battlemaster::is_portable_structure_template(victim_template)
    {
        return false;
    }
    if container.is_garrison_contain() {
        return container.is_enclosing_garrison_container();
    }
    true
}
