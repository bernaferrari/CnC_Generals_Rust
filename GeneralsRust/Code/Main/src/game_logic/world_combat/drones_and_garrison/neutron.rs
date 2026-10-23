use super::super::super::*;

impl GameLogic {
    /// Apply NeutronBlast residual at world impact: kill infantry + unman vehicles
    /// in blast radius. Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    ///
    /// Fail-closed: not full AffectAirborne / ally Relationship matrix.
    /// C++ NeutronCannonShell DumbProjectileBehavior residual (Bezier flight + blast).
    pub fn spawn_neutron_cannon_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.spawn_neutron_cannon_shell_projectile_with_effects(
            source_id,
            from,
            aim,
            intended,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_neutron_cannon_shell_projectile_with_effects(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_neutron_shell::{
            NEUTRON_CANNON_SHELL_PROJECTILE, NEUTRON_SHELL_MAX_HEALTH, neutron_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(NEUTRON_CANNON_SHELL_PROJECTILE) {
            let mut t = ThingTemplate::new(NEUTRON_CANNON_SHELL_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(NEUTRON_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(NEUTRON_CANNON_SHELL_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on NukeCannonNeutronWeapon vs infantry (**10**).
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_neutron_shell::neutron_shell_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.neutron_shell_scatter_applied =
                self.neutron_shell_scatter_applied.saturating_add(1);
        }
        // Pure-splash apply (HOST_NEUTRON_BLAST_RADIUS 70). Miss counter peels when the
        // scatter aim lands outside primary splash residual (default 10).
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_neutron_shell::neutron_shell_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_neutron_shell::NEUTRON_BLAST_DEFAULT_RADIUS {
                        self.neutron_shell_scatter_misses =
                            self.neutron_shell_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 2.0;
        let pid = self.create_object_with_script_effects(
            NEUTRON_CANNON_SHELL_PROJECTILE,
            team,
            start,
            None,
            presentation_effects,
        )?;
        let frames = neutron_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.neutron_cannon_shell_projectile = true;
            o.neutron_shell_from = Some([start.x, start.y, start.z]);
            o.neutron_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.neutron_shell_launch_frame = Some(self.frame);
            o.neutron_shell_flight_frames = frames;
            o.note_producer(source_id);
            o.set_body_max_health_with_mode(
                NEUTRON_SHELL_MAX_HEALTH,
                crate::game_logic::object::MaxHealthChange::FullyHeal,
            );
            self.health_events
                .record_applied_heal(o.id, o.health.current);
            let dir = aim - start;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        self.neutron_shells_spawned = self.neutron_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_neutron_cannon_shell_projectiles(&mut self) {
        self.update_neutron_cannon_shell_projectiles_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_neutron_cannon_shell_projectiles_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_neutron_shell::neutron_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.neutron_cannon_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, Team)> = Vec::new();
        for id in flying {
            let (source, team, from, aim, launch, total) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .neutron_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .neutron_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                let launch = o.neutron_shell_launch_frame.unwrap_or(frame);
                let total = o.neutron_shell_flight_frames.max(1);
                (o.producer_id, o.team, from, aim, launch, total)
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / total as f32).clamp(0.0, 1.0);
            let pos = neutron_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position_with_effects(pos, self.frame, presentation_effects);
                let d = pos - prev;
                if d.length_squared() > 1e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
                o.movement.velocity = d;
            }
            if elapsed >= total || t >= 0.999 {
                impact.push((id, source, aim, team));
            }
        }
        for (id, source, pos, team) in impact {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.neutron_cannon_shell_projectile = false;
                o.set_position_with_effects(pos, self.frame, presentation_effects);
            }
            // DetonateCallsKill residual: NeutronBlastBehavior on shell die.
            let caster_team = source
                .and_then(|sid| self.objects.get(&sid).map(|s| s.team))
                .unwrap_or(team);
            let _ = self.apply_neutron_blast_at_with_effects(
                pos,
                caster_team,
                source,
                true,
                presentation_effects,
            );
            self.mark_object_for_destruction_with_effects(id, Some(team), presentation_effects);
        }
    }

    pub fn honesty_neutron_shell_projectile_ok(&self) -> bool {
        self.neutron_shells_spawned > 0
    }

    /// Apply NeutronBlast residual: kill infantry, unman vehicles, and
    /// `killAllContained` on every in-radius container (including structures).
    /// Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    pub fn apply_neutron_blast_at(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
    ) -> (u32, u32, u32) {
        self.apply_neutron_blast_at_with_effects(
            impact,
            caster_team,
            caster_id,
            affect_allies,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_neutron_blast_at_with_effects(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, u32, u32) {
        use crate::game_logic::host_neutron_shell::{
            HOST_NEUTRON_BLAST_RADIUS, NEUTRON_SHELL_AUDIO, NeutronEffect,
            in_neutron_blast_radius_2d, is_legal_neutron_blast_target, neutron_effect_for_target,
        };

        let center = (impact.x, impact.z);
        let radius = HOST_NEUTRON_BLAST_RADIUS;
        let mut infantry_kills = 0u32;
        let mut vehicles_unmanned = 0u32;
        let mut vehicle_kills = 0u32;
        let mut passengers_killed = 0u32;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut bomb_detonate_ids: Vec<ObjectId> = Vec::new();
        let mut unmanned_ids: Vec<ObjectId> = Vec::new();

        let candidates: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if caster_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                if !in_neutron_blast_radius_2d(center, (pos.x, pos.z), radius) {
                    return None;
                }
                let same_team = obj.team == caster_team;
                if !is_legal_neutron_blast_target(
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Drone),
                    obj.status.airborne_target,
                    false, // AffectAirborne = No for NeutronCannonShell residual
                    same_team,
                    affect_allies,
                ) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        // C++ neutronBlastToObject: if contain → killAllContained, even on
        // structures / transports / drones that are otherwise not unmanned.
        // TunnelContain iterates the shared TunnelTracker pool, not the local door list.
        let contain_pairs: Vec<(ObjectId, Vec<ObjectId>)> = candidates
            .iter()
            .filter_map(|id| {
                let obj = self.objects.get(id)?;
                let mut occupants = obj.contained_units();
                let is_tunnel = obj.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &obj.template_name,
                    );
                if is_tunnel {
                    for uid in self
                        .tunnel_network
                        .contained_for_player(obj.tunnel_system_key())
                    {
                        if !occupants.contains(&uid) {
                            occupants.push(uid);
                        }
                    }
                }
                if occupants.is_empty() {
                    None
                } else {
                    Some((*id, occupants))
                }
            })
            .collect();

        for id in candidates {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let effect = neutron_effect_for_target(
                obj.is_kind_of(KindOf::Infantry),
                obj.is_kind_of(KindOf::Vehicle),
                obj.is_kind_of(KindOf::Drone),
                &obj.template_name,
            );
            match effect {
                NeutronEffect::KillInfantry => {
                    // Residual: kill infantry (take full health damage).
                    let receiving_damage_id = obj.id;
                    let receiving_damage_argument_1 = obj.health.current.max(1.0) * 10.0;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_id,
                        receiving_damage_argument_1,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                    let obj = self
                        .objects
                        .get_mut(&receiving_damage_id)
                        .expect("bounded residual victim remains installed");
                    if !obj.is_alive() || obj.health.current <= 0.0 {
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    } else {
                        // Force kill residual.
                        let receiving_damage_id = obj.id;
                        let _ = self.apply_residual_damage_owned_with_effects(
                            receiving_damage_id,
                            999_999.0,
                            caster_id,
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                crate::game_logic::combat::DamageType::Unresistable,
                            ),
                            None,
                            presentation_effects,
                        );
                        let obj = self
                            .objects
                            .get_mut(&receiving_damage_id)
                            .expect("bounded residual victim remains installed");
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    }
                }
                NeutronEffect::UnmanVehicle => {
                    if obj.is_car_bomb() {
                        // Dead-man trigger residual — detonate after this borrow ends.
                        bomb_detonate_ids.push(id);
                    } else {
                        obj.apply_kill_pilot_unmanned(
                            &mut self.health_events,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                        // C++ NeutonBlastBehavior.cpp:124-127 neutronBlastToObject:
                        //   getAI()->aiIdle(CMD_FROM_AI);
                        //   TheGameLogic->deselectObject(obj, PLAYERMASK_ALL, TRUE);
                        obj.set_ai_state(
                            AIState::Idle,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                        obj.deselect();
                        // C++ NeutronBlastBehavior: setTeam(neutral) residual.
                        obj.team = Team::Neutral;
                        vehicles_unmanned = vehicles_unmanned.saturating_add(1);
                        unmanned_ids.push(id);
                    }
                }
                NeutronEffect::KillVehicle => {
                    let receiving_damage_target_id = obj.id;
                    let receiving_damage_argument_1 = obj.health.current.max(1.0) * 10.0;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_target_id,
                        receiving_damage_argument_1,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                    vehicle_kills = vehicle_kills.saturating_add(1);
                    destroy_ids.push(id);
                }
                NeutronEffect::None => {}
            }
        }

        for (container_id, occupants) in contain_pairs {
            if let Some(container) = self.objects.get_mut(&container_id) {
                for &occ_id in &occupants {
                    container.remove_occupant(occ_id, &mut self.ownership_events);
                }
            }
            for occ_id in occupants {
                if destroy_ids.contains(&occ_id) {
                    continue;
                }
                let Some(occ) = self.objects.get_mut(&occ_id) else {
                    continue;
                };
                if !occ.is_alive() {
                    continue;
                }
                occ.set_contained_by(None);
                occ.set_ai_state(
                    AIState::Idle,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                let receiving_damage_id = occ.id;
                let receiving_damage_argument_1 = occ.health.current.max(1.0) * 10.0;
                let _ = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_id,
                    receiving_damage_argument_1,
                    caster_id,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                        crate::game_logic::combat::DamageType::Unresistable,
                    ),
                    None,
                    presentation_effects,
                );
                let occ = self
                    .objects
                    .get_mut(&receiving_damage_id)
                    .expect("bounded residual victim remains installed");
                if occ.is_alive() && occ.health.current > 0.0 && !occ.status.destroyed {
                    let receiving_damage_target_id = occ.id;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_target_id,
                        999_999.0,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                }
                passengers_killed = passengers_killed.saturating_add(1);
                infantry_kills = infantry_kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }

        // C++ deselectObject(PLAYERMASK_ALL): drop unmanned husks from every
        // selection roster so they cannot keep player/AI orders.
        if !unmanned_ids.is_empty() {
            for &id in &unmanned_ids {
                self.admit_local_drawable_deselected(id);
            }
            for player in self.players.values_mut() {
                player
                    .selected_objects
                    .retain(|sid| !unmanned_ids.contains(sid));
            }
        }

        for id in destroy_ids {
            self.mark_object_for_destruction_with_effects(
                id,
                Some(caster_team),
                presentation_effects,
            );
        }
        for id in bomb_detonate_ids {
            let _ = self.maybe_detonate_carbomb_on_unmanned_with_effects(id, presentation_effects);
        }

        self.neutron_shell_residual_blasts = self.neutron_shell_residual_blasts.saturating_add(1);
        self.neutron_shell_residual_infantry_kills = self
            .neutron_shell_residual_infantry_kills
            .saturating_add(infantry_kills);
        self.neutron_shell_residual_vehicles_unmanned = self
            .neutron_shell_residual_vehicles_unmanned
            .saturating_add(vehicles_unmanned);
        let _ = passengers_killed;

        self.queue_audio_event(
            AudioEventRequest::new(NEUTRON_SHELL_AUDIO)
                .with_position(impact)
                .with_priority(155),
        );
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            impact,
            Some(impact),
            self.frame,
            caster_id.unwrap_or(ObjectId(0)),
            None,
        );

        (infantry_kills, vehicles_unmanned, vehicle_kills)
    }
}
