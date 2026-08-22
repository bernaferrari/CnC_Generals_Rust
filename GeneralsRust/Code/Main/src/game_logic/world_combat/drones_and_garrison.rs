//! Host combat `impl GameLogic` — `drones_and_garrison`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;


/// C++ `getMultiLogicalBonePosition("FIREPOINT"|"STATION")` max.
const MAX_GARRISON_FIRE_POINTS: usize = 40;
/// C++ GameData `WeaponBonus = GARRISONED RANGE 133%`.
/// GarrisonContain / HelixContain `onContaining` set `WEAPONBONUSCONDITION_GARRISONED`.
const GARRISONED_WEAPON_RANGE_MULT: f32 = 1.33;
/// C++ `HelixContain::redeployOccupants` (`HelixContain.cpp:115`) `firePos.z += 8`.
/// Leftover `helix_contain.rs` already matches. Host Y-up maps C++ Z → Y.
const HELIX_OCCUPANT_FIRE_HEIGHT: f32 = 8.0;



fn cpp_bone_to_host_local(bone: gamelogic::common::Coord3D) -> glam::Vec3 {
    // C++ Z-up (x, y, z) -> host Y-up (x, z, y).
    glam::Vec3::new(bone.x, bone.z, bone.y)
}

fn rotate_yaw_host(origin: glam::Vec3, yaw: f32, local: glam::Vec3) -> glam::Vec3 {
    let (sin, cos) = yaw.sin_cos();
    glam::Vec3::new(
        origin.x + local.x * cos - local.z * sin,
        origin.y + local.y,
        origin.z + local.x * sin + local.z * cos,
    )
}

fn load_prefix_bones_for_model(
    container: &Object,
    model: &str,
    prefix: &str,
    max: usize,
) -> Vec<glam::Vec3> {
    let scale = container.thing.template.asset_scale;
    let pos = container.get_position();
    let yaw = container.get_orientation();
    let mut out = Vec::new();
    for i in 1..=max {
        let name = format!("{prefix}{i:02}");
        let Some(local) =
            gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, &name)
        else {
            break;
        };
        out.push(rotate_yaw_host(pos, yaw, cpp_bone_to_host_local(local)));
    }
    out
}

fn load_prefix_bones_world(container: &Object, prefix: &str, max: usize) -> Vec<glam::Vec3> {
    load_prefix_bones_for_model(
        container,
        container.thing.template.get_model_name(),
        prefix,
        max,
    )
}

fn named_bone_world(container: &Object, name: &str) -> Option<glam::Vec3> {
    let model = container.thing.template.get_model_name();
    let scale = container.thing.template.asset_scale;
    let pos = container.get_position();
    let yaw = container.get_orientation();
    let local = gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, name)?;
    Some(rotate_yaw_host(pos, yaw, cpp_bone_to_host_local(local)))
}

fn garrison_condition_index(
    state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
) -> u8 {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    match state {
        HostBodyDamageType::Damaged => 1,
        HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => 2,
        _ => 0,
    }
}

fn garrison_points_for_condition<'a>(
    bd: &'a crate::game_logic::BuildingData,
    idx: u8,
) -> &'a [glam::Vec3] {
    match idx {
        1 if !bd.garrison_fire_points_damaged.is_empty() => &bd.garrison_fire_points_damaged,
        2 if !bd.garrison_fire_points_really_damaged.is_empty() => {
            &bd.garrison_fire_points_really_damaged
        }
        _ => &bd.garrison_fire_points,
    }
}

fn load_garrison_condition_bone_sets(
    container: &Object,
) -> (Vec<glam::Vec3>, Vec<glam::Vec3>, Vec<glam::Vec3>) {
    let base = container.thing.template.get_model_name();
    let pristine = load_prefix_bones_for_model(container, base, "FIREPOINT", MAX_GARRISON_FIRE_POINTS);
    let dmg_key = crate::assets::mesh_asset_resolve::model_key_with_body_damage(base, 1, false);
    let rd_key = crate::assets::mesh_asset_resolve::model_key_with_body_damage(base, 2, false);
    let damaged = if dmg_key != base {
        load_prefix_bones_for_model(container, &dmg_key, "FIREPOINT", MAX_GARRISON_FIRE_POINTS)
    } else {
        Vec::new()
    };
    let really = if rd_key != base && rd_key != dmg_key {
        load_prefix_bones_for_model(container, &rd_key, "FIREPOINT", MAX_GARRISON_FIRE_POINTS)
    } else {
        Vec::new()
    };
    (pristine, damaged, really)
}

fn transport_passenger_fire_origin(container: &Object, passenger_index: usize) -> glam::Vec3 {
    // C++ HelixContain::redeployOccupants (HelixContain.cpp:112-123): every rider
    // setPosition at Helix origin z += 8, not sequential FIREPOINT bones.
    // Humvee/Chinook/Bus keep OpenContain FIREPOINT (hq-ncs1d).
    if container.is_helix_transport {
        let mut fire_pos = container.get_position();
        fire_pos.y += HELIX_OCCUPANT_FIRE_HEIGHT;
        return fire_pos;
    }
    let bones = load_prefix_bones_world(container, "FIREPOINT", MAX_GARRISON_FIRE_POINTS);
    if bones.is_empty() {
        container.get_position()
    } else {
        bones[passenger_index % bones.len()]
    }
}

fn open_contain_exit_path(
    container: &Object,
    which_path: u8,
    number_exits: i32,
) -> (glam::Vec3, glam::Vec3, u8) {
    let origin = container.get_position();
    let yaw = container.get_orientation();
    let geom = container.thing.template.geometry_info;
    let major = if geom.authored {
        geom.major_radius.max(8.0)
    } else {
        20.0
    };
    let fallback_end = {
        let (sin, cos) = yaw.sin_cos();
        glam::Vec3::new(origin.x + major * cos, origin.y, origin.z + major * sin)
    };
    // C++ OpenContain::exitObjectViaDoor: numberExits<=0 skips the door walk.
    if number_exits <= 0 {
        return (origin, origin, 1);
    }
    // C++ numberExits>1 uses ExitStart0N/ExitEnd0N cycling m_whichExitPath.
    if number_exits > 1 {
        let n = number_exits as u8;
        let idx = if which_path == 0 {
            1
        } else {
            ((which_path - 1) % n) + 1
        };
        let start = named_bone_world(container, &format!("ExitStart{idx:02}")).unwrap_or(origin);
        let end = named_bone_world(container, &format!("ExitEnd{idx:02}")).unwrap_or(fallback_end);
        let next = (idx % n) + 1;
        return (start, end, next);
    }
    let start = named_bone_world(container, "ExitStart").unwrap_or(origin);
    let end = named_bone_world(container, "ExitEnd").unwrap_or(fallback_end);
    (start, end, 1)
}

fn closest_free_garrison_point(
    points: &[glam::Vec3],
    occupied: &[Option<ObjectId>],
    occupant_id: ObjectId,
    target: glam::Vec3,
    fallback: glam::Vec3,
) -> (usize, glam::Vec3) {
    if points.is_empty() {
        return (0, fallback);
    }
    let mut best_i = 0;
    let mut best_d = f32::MAX;
    let mut best = points[0];
    for (i, p) in points.iter().enumerate() {
        let taken = occupied.get(i).and_then(|id| *id);
        if taken.is_some() && taken != Some(occupant_id) {
            continue;
        }
        let d = (*p - target).length_squared();
        if d < best_d {
            best_d = d;
            best_i = i;
            best = *p;
        }
    }
    (best_i, best)
}

fn garrison_occupant_fire_point(
    container: &Object,
    occupant_id: ObjectId,
    target_pos: glam::Vec3,
) -> (usize, glam::Vec3) {
    let fallback = container.get_position();
    let Some(bd) = container.building_data.as_ref() else {
        return (0, fallback);
    };
    // C++ WeaponSet.cpp:632-633 / GarrisonContain.cpp:662-663:
    // non-enclosing Fire Base does not use FIREPOINTs. Occupants fire from
    // their pre-assigned STATION bone (not the building center).
    if !container.is_enclosing_garrison_container() {
        return station_occupant_fire_point(bd, occupant_id, fallback);
    }
    let idx = garrison_condition_index(container.body_damage_state);
    closest_free_garrison_point(
        garrison_points_for_condition(bd, idx),
        &bd.garrison_point_occupant,
        occupant_id,
        target_pos,
        fallback,
    )
}

/// C++ `positionObjectsAtStationGarrisonPoints` / `pickAStationForMe`.
fn station_occupant_fire_point(
    bd: &crate::game_logic::BuildingData,
    occupant_id: ObjectId,
    fallback: glam::Vec3,
) -> (usize, glam::Vec3) {
    if bd.garrison_station_points.is_empty() {
        return (0, fallback);
    }
    for (i, slot) in bd.garrison_point_occupant.iter().enumerate() {
        if *slot == Some(occupant_id) {
            if let Some(&pos) = bd.garrison_station_points.get(i) {
                return (i, pos);
            }
        }
    }
    for (i, pos) in bd.garrison_station_points.iter().enumerate() {
        let taken = bd.garrison_point_occupant.get(i).and_then(|id| *id);
        if taken.is_none() {
            return (i, *pos);
        }
    }
    (0, bd.garrison_station_points[0])
}

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
        use crate::game_logic::host_neutron_shell::{
            neutron_shell_flight_frames, NEUTRON_CANNON_SHELL_PROJECTILE, NEUTRON_SHELL_MAX_HEALTH,
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
        let pid = self.create_object(NEUTRON_CANNON_SHELL_PROJECTILE, team, start)?;
        let frames = neutron_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.neutron_cannon_shell_projectile = true;
            o.neutron_shell_from = Some([start.x, start.y, start.z]);
            o.neutron_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.neutron_shell_launch_frame = Some(self.frame);
            o.neutron_shell_flight_frames = frames;
            o.note_producer(source_id);
            o.health.maximum = NEUTRON_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, NEUTRON_SHELL_MAX_HEALTH);
            let dir = aim - start;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        self.neutron_shells_spawned = self.neutron_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_neutron_cannon_shell_projectiles(&mut self) {
        use crate::game_logic::host_neutron_shell::neutron_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
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
                o.set_position(pos);
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.neutron_cannon_shell_projectile = false;
                o.set_position(pos);
            }
            // DetonateCallsKill residual: NeutronBlastBehavior on shell die.
            let caster_team = source
                .and_then(|sid| self.objects.get(&sid).map(|s| s.team))
                .unwrap_or(team);
            let _ = self.apply_neutron_blast_at(pos, caster_team, source, true);
            self.mark_object_for_destruction(id, Some(team));
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

        use crate::game_logic::host_neutron_shell::{
            in_neutron_blast_radius_2d, is_legal_neutron_blast_target, neutron_effect_for_target,
            NeutronEffect, HOST_NEUTRON_BLAST_RADIUS, NEUTRON_SHELL_AUDIO,
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
            .iter()
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
                    for uid in self.tunnel_network.contained_for_player(obj.tunnel_system_key()) {
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
                    let _ = obj.take_damage_from(obj.health.current.max(1.0) * 10.0, caster_id);
                    if !obj.is_alive() || obj.health.current <= 0.0 {
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    } else {
                        // Force kill residual.
                        let _ = obj.take_damage_from(999_999.0, caster_id);
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    }
                }
                NeutronEffect::UnmanVehicle => {
                    if obj.is_car_bomb() {
                        // Dead-man trigger residual — detonate after this borrow ends.
                        bomb_detonate_ids.push(id);
                    } else {
                        obj.apply_kill_pilot_unmanned();
                        // C++ NeutonBlastBehavior.cpp:124-127 neutronBlastToObject:
                        //   getAI()->aiIdle(CMD_FROM_AI);
                        //   TheGameLogic->deselectObject(obj, PLAYERMASK_ALL, TRUE);
                        obj.set_ai_state(AIState::Idle);
                        obj.deselect();
                        // C++ NeutronBlastBehavior: setTeam(neutral) residual.
                        obj.team = Team::Neutral;
                        vehicles_unmanned = vehicles_unmanned.saturating_add(1);
                        unmanned_ids.push(id);
                    }
                }
                NeutronEffect::KillVehicle => {
                    let _ = obj.take_damage_from(obj.health.current.max(1.0) * 10.0, caster_id);
                    vehicle_kills = vehicle_kills.saturating_add(1);
                    destroy_ids.push(id);
                }
                NeutronEffect::None => {}
            }
        }

        for (container_id, occupants) in contain_pairs {
            if let Some(container) = self.objects.get_mut(&container_id) {
                for &occ_id in &occupants {
                    container.remove_occupant(occ_id);
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
                occ.set_ai_state(AIState::Idle);
                let _ = occ.take_damage_from(occ.health.current.max(1.0) * 10.0, caster_id);
                if occ.is_alive() && occ.health.current > 0.0 && !occ.status.destroyed {
                    let _ = occ.take_damage_from(999_999.0, caster_id);
                }
                passengers_killed = passengers_killed.saturating_add(1);
                infantry_kills = infantry_kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }


        // C++ deselectObject(PLAYERMASK_ALL): drop unmanned husks from every
        // selection roster so they cannot keep player/AI orders.
        if !unmanned_ids.is_empty() {
            self.selected_objects
                .retain(|sid| !unmanned_ids.contains(sid));
            for player in self.players.values_mut() {
                player
                    .selected_objects
                    .retain(|sid| !unmanned_ids.contains(sid));
            }
        }


        for id in destroy_ids {
            self.mark_object_for_destruction(id, Some(caster_team));
        }
        for id in bomb_detonate_ids {
            let _ = self.maybe_detonate_carbomb_on_unmanned(id);
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

    /// Residual fire-from-transport: docked passengers auto-engage nearest
    /// enemy in weapon range. HelixContain riders fire from hull origin +8 Y
    /// (`HelixContain::redeployOccupants`). Other transports use sequential
    /// `FIREPOINT` bones (`OpenContain::putObjAtNextFirePoint`); hull if none.
    pub(in super::super) fn try_transport_passenger_residual_fire(
        &mut self,
        passenger_id: ObjectId,
    ) {
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&passenger_id) else {
            return;
        };
        if !attacker.is_alive() || attacker.weapon.is_none() {
            return;
        }
        let Some(weapon) = attacker.weapon.as_ref() else {
            return;
        };
        if !Object::weapon_ready(weapon, current_time) {
            return;
        }

        let container_id = attacker.container_id();
        let Some(cid) = container_id else {
            return;
        };
        let Some(container) = self.objects.get(&cid) else {
            return;
        };
        // C++ OverlordContain::isPassengerAllowedToFire — nested contain voids fire.
        let nested = container.contained_by.is_some();
        let bunker_slots = container.overlord_bunker_slot_capacity();
        let bunker_may = crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
            bunker_slots,
            nested,
        );
        // C++ OpenContain::isPassengerAllowedToFire residual + Overlord bunker peel.
        if !container.passengers_allowed_to_fire && !bunker_may {
            return;
        }
        if nested {
            return;
        }
        // C++ TransportContain::isPassengerAllowedToFire (TransportContain.cpp:576-578):
        // leftover helper — only infantry fire out. Vehicles ride silent
        // (Combat Chinook AllowInsideKindOf = INFANTRY VEHICLE).
        if !gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(
            attacker.is_kind_of(KindOf::Infantry),
        ) {
            return;
        }

        let is_battle_bus = container.is_battle_bus_style_container();
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let team = attacker.team;
        // HelixContain::onContaining sets WEAPONBONUSCONDITION_GARRISONED.
        // Occupants stay Docked, so Object::weapon_bonus_fields never applies
        // the Garrisoned AIState 133% path used by bunker occupants.
        let range = if container.is_helix_transport {
            weapon.range * GARRISONED_WEAPON_RANGE_MULT
        } else {
            weapon.range
        };
        let damage = weapon.damage;
        let passenger_index = container
            .contained_units()
            .iter()
            .position(|&id| id == passenger_id)
            .unwrap_or(0);
        let fire_pos = transport_passenger_fire_origin(container, passenger_index);
        if bunker_may {
            if let Some(c) = self.objects.get_mut(&cid) {
                if !c.passengers_allowed_to_fire {
                    c.passengers_allowed_to_fire = true;
                    c.record_host_stealth_flags();
                }
            }
        }

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter(|(id, _)| **id != passenger_id && **id != cid)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            passenger_id,
            team,
            fire_pos,
            candidates,
            |_| range,
            |c| c.is_alive && c.team != team && !c.is_neutral && c.combat_kind,
        );

        let Some((target_id, _, _)) = best else {
            return;
        };

        let weapon_snap = self
            .objects
            .get(&passenger_id)
            .and_then(|a| a.weapon.clone());
        let (destroyed, _) = self.residual_auto_fire_apply_damage(
            passenger_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            0,
        );

        if let Some(attacker) = self.objects.get_mut(&passenger_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                0,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon.as_mut() {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon
                    .as_ref()
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    0,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(passenger_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(passenger_id, 2);
            }
            // Kill XP awarded after this borrow via award_experience.
        }
        // Contained fire changes where the shot originates, not which
        // concrete passenger WeaponSet slot discharged.
        let _ = self.record_accepted_weapon_discharge(passenger_id, 0);

        if destroyed {
            self.award_score_the_kill_experience(passenger_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
        }

        if is_battle_bus {
            self.battle_bus.record_passenger_fire();
        } else if is_combat_chinook {
            self.combat_chinook.record_passenger_fire();
        } else if is_listening_outpost {
            self.listening_outpost.record_passenger_fire();
        }
    }

    /// Residual fire-from-garrison: enclosing occupants fire from a FIREPOINT
    /// bone (C++ `calcBestGarrisonPosition`). Non-enclosing Fire Base fires
    /// from the occupant's pre-assigned STATION bone, not the building center.
    pub(in super::super) fn try_garrison_residual_fire(&mut self, garrisoned_id: ObjectId) {
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&garrisoned_id) else {
            return;
        };
        if !attacker.is_alive() {
            return;
        }
        let container_id = attacker.container_id();
        if container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|container| container.status.disabled_subdued)
        {
            // C++ GarrisonContain::isPassengerAllowedToFire: DISABLED_SUBDUED
            // (flashbang / neutron) silences window fire.
            return;
        }
        let has_any_weapon = attacker.weapon_slot(0).is_some()
            || attacker.weapon_slot(1).is_some()
            || attacker.weapon_slot(2).is_some();
        if !has_any_weapon {
            return;
        }

        let team = attacker.team;
        if let Some(cid) = container_id {
            self.ensure_garrison_bones(cid);
        }
        let ordered_target = container_id.and_then(|cid| self.objects.get(&cid).and_then(|c| c.target));
        let occupants = container_id
            .and_then(|cid| self.objects.get(&cid).map(|c| c.contained_units()))
            .unwrap_or_default();
        let occupant_index = occupants
            .iter()
            .position(|&id| id == garrisoned_id)
            .unwrap_or(0);

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter(|(id, _)| **id != garrisoned_id && Some(**id) != container_id)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();

        // C++ GarrisonContain: occupant getCurrentWeapon + best FIREPOINT vs victim.
        let mut best: Option<(ObjectId, f32, u8, glam::Vec3, f32, usize)> = None;
        for cand in &candidates {
            if !(cand.is_alive && cand.team != team && !cand.is_neutral && cand.combat_kind) {
                continue;
            }
            // C++ PartitionFilter / AcquirePlayerTargets: undetected stealth
            // is not a legal auto-acquire victim (transport path already
            // skips via pick_nearest_residual_target).
            if cand.effectively_stealthed {
                continue;
            }
            if let Some(ordered) = ordered_target {
                if cand.id != ordered {
                    continue;
                }
            }
            let (point_index, fire_pos) = container_id
                .and_then(|cid| self.objects.get(&cid))
                .map(|container| {
                    garrison_occupant_fire_point(container, garrisoned_id, cand.position)
                })
                .unwrap_or((occupant_index, cand.position));
            let Some(target_obj) = self.objects.get(&cand.id) else {
                continue;
            };
            let Some(attacker) = self.objects.get(&garrisoned_id) else {
                return;
            };
            let slot = attacker
                .select_combat_weapon_slot(target_obj, current_time)
                .or_else(|| {
                    let s = attacker.active_weapon_slot;
                    attacker
                        .weapon_slot(s)
                        .filter(|w| Object::weapon_ready(w, current_time))
                        .map(|_| s)
                });
            let Some(slot) = slot else {
                continue;
            };
            let Some(weapon) = attacker.weapon_slot(slot) else {
                continue;
            };
            if !Object::weapon_ready(weapon, current_time) {
                continue;
            }
            // C++ Weapon::isWithinAttackRange / getAttackRange applies
            // WEAPONBONUSCONDITION_GARRISONED RANGE 133% (not raw PrimaryAttackRange).
            let range = attacker.effective_weapon_range(weapon.range);
            let dist = fire_pos.distance(cand.position);
            if dist > range {
                continue;
            }
            if best.as_ref().map(|(_, d, _, _, _, _)| dist < *d).unwrap_or(true) {
                best = Some((cand.id, dist, slot, fire_pos, weapon.damage, point_index));
            }
        }

        let Some((target_id, _, slot, fire_pos, damage, point_index)) = best else {
            return;
        };

        if let Some(cid) = container_id {
            if let Some(container) = self.objects.get_mut(&cid) {
                if let Some(bd) = container.building_data.as_mut() {
                    if bd.garrison_point_occupant.len() <= point_index {
                        bd.garrison_point_occupant.resize(point_index + 1, None);
                    }
                    bd.garrison_point_occupant[point_index] = Some(garrisoned_id);
                }
            }
            // C++ positionObjectsAtStationGarrisonPoints: stay on STATION.
            let pin_station = self
                .objects
                .get(&cid)
                .is_some_and(|c| !c.is_enclosing_garrison_container());
            if pin_station {
                if let Some(occ) = self.objects.get_mut(&garrisoned_id) {
                    occ.set_position(fire_pos);
                }
            }
        }

        let weapon_snap = self
            .objects
            .get(&garrisoned_id)
            .and_then(|a| a.weapon_slot(slot).cloned());
        let (destroyed, _) = self.residual_auto_fire_apply_damage(
            garrisoned_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            slot,
        );

        if let Some(attacker) = self.objects.get_mut(&garrisoned_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon_slot_mut(slot) {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon_slot(slot)
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    slot,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(garrisoned_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(garrisoned_id, 2);
            }
            // Kill XP awarded after this borrow via award_experience.
        }
        // Occupant discharges their own current slot from the FIREPOINT offset.
        let _ = self.record_accepted_weapon_discharge(garrisoned_id, slot);

        if destroyed {
            self.award_score_the_kill_experience(garrisoned_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
        }
        self.garrison_residual_fires = self.garrison_residual_fires.saturating_add(1);
        self.ensure_garrison_gun_effect(container_id, point_index, fire_pos);
    }

    /// C++ GarrisonContain::onContaining setTeam + academy + CAN_ATTACK + stations.
    /// C++ GarrisonContain::onObjectCreated InitialRoster spawn + addToContain.
    pub(in super::super) fn apply_garrison_initial_roster(
        &mut self,
        container_id: ObjectId,
        team: Team,
        position: glam::Vec3,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if container.thing.template.contain_module.kind != ContainModuleKind::Garrison {
            return;
        }
        let roster = gamelogic::object::contain::InitialRoster {
            template_name: container
                .thing
                .template
                .contain_module
                .initial_roster_template
                .clone(),
            count: container.thing.template.contain_module.initial_roster_count,
        };
        if !roster.is_populated() {
            return;
        }
        if !self.templates.contains_key(&roster.template_name) {
            return;
        }
        let payload_name = roster.template_name;
        for _ in 0..roster.count {
            let Some(occupant_id) = self.create_object(&payload_name, team, position) else {
                break;
            };
            let added = self
                .objects
                .get_mut(&container_id)
                .is_some_and(|container| container.add_occupant(occupant_id));
            if !added {
                continue;
            }
            self.tunnel_network
                .stamp_contained_by_frame(occupant_id, self.frame);
            if let Some(occupant) = self.objects.get_mut(&occupant_id) {
                occupant.set_contained_by(Some(container_id));
                occupant.set_position(position);
                occupant.stop_moving();
                occupant.set_status_moving(false);
                occupant.set_ai_state(AIState::Garrisoned);
            }
            self.apply_garrison_contain_on_enter(container_id, occupant_id);
            self.stamp_player_who_entered(container_id, occupant_id);
        }
    }

    /// C++ OpenContain::addToContain `m_playerEnteredMask = rider->getControllingPlayer()`.
    /// Sticky last enterer; not cleared on exit.
    pub(in super::super) fn stamp_player_who_entered(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let name = {
            let Some(occupant) = self.objects.get(&occupant_id) else {
                return;
            };
            if let Some(pid) = occupant.owner_player_id {
                self.player_name(pid)
            } else {
                let team = occupant.team;
                self.players
                    .values()
                    .find(|p| p.team == team)
                    .map(|p| p.name.clone())
            }
        };
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            return;
        };
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.player_who_entered = name;
        }
    }

    pub(in super::super) fn apply_garrison_contain_on_enter(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.is_garrison_contain() {
            return;
        }
        self.ensure_garrison_bones(container_id);
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.set_garrison_can_attack(true);
        }
        self.place_occupant_at_garrison_station(container_id, occupant_id);
        self.stamp_player_who_entered(container_id, occupant_id);
        self.recalc_garrison_apparent_controller(container_id);
        let occupant_owner = self
            .objects
            .get(&occupant_id)
            .and_then(|o| o.owner_player_id);
        let occupant_team = self.objects.get(&occupant_id).map(|o| o.team);
        if let Some(pid) = occupant_owner {
            if let Some(player) = self.players.get_mut(&pid) {
                player.record_building_garrisoned();
            }
        } else if let Some(team) = occupant_team {
            if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                player.record_building_garrisoned();
            }
        }
    }

    /// C++ loadGarrisonPoints / loadStationGarrisonPoints.
    fn ensure_garrison_bones(&mut self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.is_garrison_contain() {
            return;
        }
        let enclosing = container.is_enclosing_garrison_container();
        let already = container
            .building_data
            .as_ref()
            .is_some_and(|b| b.garrison_points_initialized);
        if !already {
            let (pristine, damaged, really) = if enclosing {
                load_garrison_condition_bone_sets(container)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
            let stations = if enclosing {
                Vec::new()
            } else {
                let max = container
                    .thing
                    .template
                    .contain_module
                    .slots
                    .unwrap_or(MAX_GARRISON_FIRE_POINTS)
                    .min(MAX_GARRISON_FIRE_POINTS);
                load_prefix_bones_world(container, "STATION", max)
            };
            if let Some(container) = self.objects.get_mut(&container_id) {
                if let Some(bd) = container.building_data.as_mut() {
                    if enclosing {
                        bd.garrison_fire_points = pristine;
                        bd.garrison_fire_points_damaged = damaged;
                        bd.garrison_fire_points_really_damaged = really;
                        bd.garrison_point_occupant
                            .resize(bd.garrison_fire_points.len(), None);
                    } else {
                        bd.garrison_station_points = stations;
                        bd.garrison_point_occupant
                            .resize(bd.garrison_station_points.len(), None);
                    }
                    bd.garrison_points_initialized = true;
                }
            }
        }
        // C++ findConditionIndex + redeployOccupants is enclosing-only.
        // Non-enclosing Fire Base keeps pre-assigned STATION occupants.
        if enclosing {
            let idx = self
                .objects
                .get(&container_id)
                .map(|c| garrison_condition_index(c.body_damage_state))
                .unwrap_or(0);
            if let Some(container) = self.objects.get_mut(&container_id) {
                if let Some(bd) = container.building_data.as_mut() {
                    if bd.garrison_points_condition != idx {
                        bd.garrison_points_condition = idx;
                        for slot in &mut bd.garrison_point_occupant {
                            *slot = None;
                        }
                        let n = garrison_points_for_condition(bd, idx).len();
                        if n > 0 {
                            bd.garrison_point_occupant.resize(n, None);
                        }
                    }
                }
            }
        }
    }

    /// C++ pickAStationForMe + positionObjectsAtStationGarrisonPoints.
    fn place_occupant_at_garrison_station(&mut self, container_id: ObjectId, occupant_id: ObjectId) {
        let enclosing = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_enclosing_garrison_container());
        if enclosing {
            return;
        }
        let station = {
            let Some(container) = self.objects.get_mut(&container_id) else {
                return;
            };
            let Some(bd) = container.building_data.as_mut() else {
                return;
            };
            let mut chosen = None;
            for (i, slot) in bd.garrison_point_occupant.iter().enumerate() {
                if *slot == Some(occupant_id) {
                    chosen = bd.garrison_station_points.get(i).copied();
                    break;
                }
            }
            if chosen.is_none() {
                for (i, slot) in bd.garrison_point_occupant.iter_mut().enumerate() {
                    if slot.is_none() {
                        *slot = Some(occupant_id);
                        chosen = bd.garrison_station_points.get(i).copied();
                        break;
                    }
                }
            }
            chosen
        };
        if let Some(pos) = station {
            if let Some(occ) = self.objects.get_mut(&occupant_id) {
                occ.set_position(pos);
            }
        }
    }

    /// C++ ScriptActions::doNamedSetGarrisonEvacDisposition.
    pub fn set_named_garrison_evac_disposition(&mut self, unit_name: &str, disposition: u32) -> bool {
        gamelogic::object::contain::record_named_evac_disposition(unit_name, disposition);
        let Some(id) = self.find_object_id_by_name(unit_name) else {
            return false;
        };
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_garrison_evac_disposition(disposition as u8);
            return true;
        }
        false
    }

    /// C++ GarrisonContain::recalcApparentControllingPlayer.
    pub(in super::super) fn recalc_garrison_apparent_controller(&mut self, container_id: ObjectId) {
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        if occupants.is_empty() {
            if let Some(container) = self.objects.get_mut(&container_id) {
                container.restore_garrison_original_team_if_empty();
            }
            return;
        }
        let first = occupants
            .first()
            .and_then(|id| self.objects.get(id))
            .map(|o| (o.team, o.owner_player_id, o.status.detected));
        let Some((first_team, first_owner, first_detected)) = first else {
            return;
        };
        let stealth_kind_count = occupants
            .iter()
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison))
            })
            .count();
        let hide = !first_detected && stealth_kind_count == occupants.len();
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.original_team.is_none() {
                    bd.original_team = Some(container.team);
                }
                bd.hide_garrisoned_state = hide;
            }
            container.set_team_and_owner(first_team, first_owner);
        }
    }

    /// C++ `StealthUpdate.cpp:786-801` — DETECTED flip on a contained rider
    /// calls `GarrisonContain::recalcApparentControllingPlayer`.
    pub(in super::super) fn recalc_garrisons_after_occupant_detect_change(
        &mut self,
        container_ids: &[ObjectId],
    ) {
        let mut seen: Vec<ObjectId> = Vec::new();
        for &cid in container_ids {
            if seen.contains(&cid) {
                continue;
            }
            seen.push(cid);
            if self
                .objects
                .get(&cid)
                .is_some_and(|c| c.is_garrison_contain())
            {
                self.recalc_garrison_apparent_controller(cid);
            }
        }
    }

    /// C++ OpenContain::onCollide: eject other-player riders (STEALTH_GARRISON
    /// markAsDetected + aiExit) before the arriver boards.
    pub(in super::super) fn kick_other_controller_occupants_for_enter(
        &mut self,
        container_id: ObjectId,
        arriver_id: ObjectId,
    ) {
        let arriver_owner = self.objects.get(&arriver_id).and_then(|o| o.owner_player_id);
        let arriver_team = self.objects.get(&arriver_id).map(|o| o.team);
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        let mut kick: Vec<ObjectId> = Vec::new();
        for pid in occupants {
            if pid == arriver_id {
                continue;
            }
            let Some(occ) = self.objects.get(&pid) else {
                continue;
            };
            let same = match (arriver_owner, occ.owner_player_id) {
                (Some(a), Some(b)) => a == b,
                _ => arriver_team == Some(occ.team),
            };
            if !same {
                kick.push(pid);
            }
        }
        if kick.is_empty() {
            return;
        }
        let now = self.frame;
        for pid in kick {
            let stealth_garrison = self
                .objects
                .get(&pid)
                .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison));
            let delay = self
                .objects
                .get(&pid)
                .map(|o| o.stealth_delay_frames)
                .unwrap_or(0)
                .max(60);
            if stealth_garrison {
                if let Some(occ) = self.objects.get_mut(&pid) {
                    occ.mark_detected(now.saturating_add(delay));
                }
            }
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant(pid);
            }
            self.walk_unit_via_open_contain_exit(pid, container_id);
        }
        if self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain())
        {
            self.recalc_garrison_apparent_controller(container_id);
        }
    }

    /// C++ OpenContain::exitObjectViaDoor — ExitStart/End + follow-path.
    /// TransportContain::onRemoving then matches hull orientation, GoAggressiveOnExit,
    /// airborne setAllowToFall, and KeepContainerVelocityOnExit (hull motive).
    pub(in super::super) fn walk_unit_via_open_contain_exit(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
    ) {
        let unit_pos = self.objects.get(&unit_id).map(|u| u.get_position());
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let go_aggressive = container.transport_go_aggressive_on_exit();
        let airborne = container.is_above_terrain_for_exit();
        let hull_vel = container.movement.velocity;
        let yaw = container.get_orientation();
        let rally = container
            .building_data
            .as_ref()
            .and_then(|b| b.rally_point);
        let is_garrison = container.is_garrison_contain();
        let (start, end, next) = if is_garrison {
            let origin = container.get_position();
            let geom = container.thing.template.geometry_info;
            let major = if geom.authored {
                geom.major_radius.max(8.0)
            } else {
                20.0
            };
            let enclosing = container.is_enclosing_garrison_container();
            let (sin, cos) = yaw.sin_cos();
            let dest = glam::Vec3::new(origin.x + major * cos, origin.y, origin.z + major * sin);
            let start = if enclosing {
                origin
            } else {
                unit_pos.unwrap_or(origin)
            };
            (start, dest, 0u8)
        } else {
            let which = if container.which_exit_path > 0 {
                container.which_exit_path
            } else {
                container
                    .building_data
                    .as_ref()
                    .map(|b| b.which_exit_path)
                    .unwrap_or(0)
            };
            let number_exits = container.transport_number_of_exit_paths();
            open_contain_exit_path(container, which, number_exits)
        };
        // C++ exitPath = [end, end, rally?]. Live dest is rally after the door.
        let dest = if is_garrison {
            end
        } else {
            rally.unwrap_or(end)
        };
        if next > 0 {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.which_exit_path = next;
                if let Some(bd) = c.building_data.as_mut() {
                    bd.which_exit_path = next;
                }
            }
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_contained_by(None);
            unit.target = None;
            unit.set_position(start);
            unit.set_orientation(yaw);
            unit.set_destination(dest);
            unit.set_ai_state(AIState::Moving);
            unit.status.moving = true;
            if is_garrison {
                unit.stamp_safe_occlusion_frame(self.frame);
            }
            // C++ OpenContain::exitObjectViaDoor: ignoreObstacle(NULL) +
            // setIgnoreCollisionTime(LOGICFRAMES_PER_SECOND).
            unit.ignore_collisions_with = None;
            unit.ignore_collisions_until_frame = self.frame.saturating_add(30);
            if go_aggressive {
                unit.set_ai_attitude(
                    crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
                );
            }
            if airborne {
                // C++ onRemoving: isAboveTerrain → setAllowToFall; keep hull vel
                // so airborne unloads do not freeze at the door.
                unit.allow_to_fall = true;
                let mass = unit.physics_get_mass();
                unit.apply_motive_force(hull_vel * mass);
            }
        }
    }


    /// C++ putObjectAtGarrisonPoint + updateEffects GarrisonGun / FIRING_A.
    fn ensure_garrison_gun_effect(
        &mut self,
        container_id: Option<ObjectId>,
        point_index: usize,
        pos: glam::Vec3,
    ) {
        const MUZZLE_FLASH_LIFETIME: u32 = 30 / 7;
        let Some(cid) = container_id else {
            return;
        };
        self.expire_garrison_gun_muzzle_flashes(cid, MUZZLE_FLASH_LIFETIME);
        let existing = self
            .objects
            .get(&cid)
            .and_then(|c| c.building_data.as_ref())
            .and_then(|b| b.garrison_guns.get(point_index))
            .and_then(|g| g.drawable_id);
        let gun_id = existing.or_else(|| {
            if !self.templates.contains_key("GarrisonGun") {
                return None;
            }
            let team = self
                .objects
                .get(&cid)
                .map(|c| c.team)
                .unwrap_or(Team::Neutral);
            self.create_object("GarrisonGun", team, pos)
        });
        if let Some(gid) = gun_id {
            if let Some(gun) = self.objects.get_mut(&gid) {
                gun.set_position(pos);
                gun.model_condition_bits |=
                    1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A;
            }
        }
        if let Some(container) = self.objects.get_mut(&cid) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.garrison_guns.len() <= point_index {
                    bd.garrison_guns
                        .resize(point_index + 1, crate::game_logic::GarrisonGunEffect::default());
                }
                let gun = &mut bd.garrison_guns[point_index];
                gun.drawable_id = gun_id;
                gun.last_effect_frame = self.frame;
                gun.firing = true;
            }
        }
    }

    fn expire_garrison_gun_muzzle_flashes(&mut self, container_id: ObjectId, lifetime: u32) {
        let frame = self.frame;
        let mut expire_ids = Vec::new();
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                for gun in &mut bd.garrison_guns {
                    if gun.firing && frame.saturating_sub(gun.last_effect_frame) > lifetime {
                        gun.firing = false;
                        if let Some(id) = gun.drawable_id {
                            expire_ids.push(id);
                        }
                    }
                }
            }
        }
        for id in expire_ids {
            if let Some(gun) = self.objects.get_mut(&id) {
                gun.model_condition_bits &=
                    !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A);
            }
        }
    }

    /// Residual honesty: enter → garrisoned → exit path was exercised.
    pub fn honesty_garrison_enter_exit_ok(&self) -> bool {
        self.garrison_residual_enters > 0 && self.garrison_residual_exits > 0
    }

    /// Residual honesty: at least one fire-from-garrison residual shot.
    pub fn honesty_garrison_fire_ok(&self) -> bool {
        self.garrison_residual_fires > 0
    }

    /// Residual honesty: load → docked → unload path was exercised.
    pub fn honesty_transport_load_unload_ok(&self) -> bool {
        self.transport_residual_loads > 0 && self.transport_residual_unloads > 0
    }

    /// Residual honesty: Overlord BattleBunker enter → docked → exit path.
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn.
    pub fn honesty_overlord_bunker_enter_exit_ok(&self) -> bool {
        self.overlord_bunker_residual_enters > 0 && self.overlord_bunker_residual_exits > 0
    }

    // -----------------------------------------------------------------------
    // Mine / demo-trap / timed demo-charge residual
    // Fail-closed: not full MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate.
    // -----------------------------------------------------------------------

    /// Residual honesty: at least one mine/trap/charge was placed.
    pub fn mine_residual_places(&self) -> u32 {
        self.mine_residual_places
    }

    /// Residual honesty: proximity-triggered detonations.
    pub fn mine_residual_proximity_detonations(&self) -> u32 {
        self.mine_residual_proximity_detonations
    }

    /// Residual honesty: timed-charge detonations.
    pub fn mine_residual_timed_detonations(&self) -> u32 {
        self.mine_residual_timed_detonations
    }

    /// Residual honesty: manual detonations (demo trap command residual).
    pub fn mine_residual_manual_detonations(&self) -> u32 {
        self.mine_residual_manual_detonations
    }

    /// Residual honesty: dozer/worker safe mine clears (disarm without detonation).
    pub fn mine_residual_clears(&self) -> u32 {
        self.mine_residual_clears
    }

    /// Residual honesty: place → enemy trigger → damage path exercised.
    pub fn honesty_mine_place_trigger_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_proximity_detonations > 0
    }

    /// Residual honesty: place timed charge → detonation path exercised.
    pub fn honesty_timed_demo_charge_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_timed_detonations > 0
    }

    /// Residual honesty: place enemy mine → dozer clear → mine gone, dozer lives.
    pub fn honesty_mine_clear_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_clears > 0
    }

    /// Residual dozer structure-repair command accepts.
    pub fn repair_residual_structure_commands(&self) -> u32 {
        self.repair_residual_structure_commands
    }

    /// Residual structure HP heal ticks applied by dozer Repairing state.
    pub fn repair_residual_structure_heals(&self) -> u32 {
        self.repair_residual_structure_heals
    }

    /// Residual vehicle/aircraft SeekingRepair heal ticks at pad/war-factory/airfield.
    pub fn repair_residual_vehicle_heals(&self) -> u32 {
        self.repair_residual_vehicle_heals
    }

    /// Record a successful dozer structure Repair command acceptance.
    pub fn record_structure_repair_residual_command(&mut self) {
        self.repair_residual_structure_commands =
            self.repair_residual_structure_commands.saturating_add(1);
    }

    /// Record a structure HP heal tick from dozer Repairing residual.
    pub fn record_structure_repair_residual_heal(&mut self) {
        self.repair_residual_structure_heals =
            self.repair_residual_structure_heals.saturating_add(1);
    }

    /// Record a vehicle/aircraft pad heal tick from SeekingRepair residual.
    pub fn record_vehicle_repair_residual_heal(&mut self) {
        self.repair_residual_vehicle_heals = self.repair_residual_vehicle_heals.saturating_add(1);
    }

    /// Residual structure repair honesty: command issued and at least one HP heal tick.
    /// Fail-closed: not full C++ percent-heal / sole-benefactor / scaffolding parity.
    pub fn honesty_structure_repair_ok(&self) -> bool {
        self.repair_residual_structure_commands > 0 && self.repair_residual_structure_heals > 0
    }

    /// Residual vehicle pad repair honesty: at least one SeekingRepair heal tick.
    /// Fail-closed: not full RepairDockUpdate TimeForFullHeal / dock bones parity.
    pub fn honesty_vehicle_repair_ok(&self) -> bool {
        self.repair_residual_vehicle_heals > 0
    }

    /// Combined host repair residual path honesty (structure or vehicle pad).
    pub fn honesty_repair_ok(&self) -> bool {
        self.honesty_structure_repair_ok() || self.honesty_vehicle_repair_ok()
    }

    /// Residual ambulance AutoHeal infantry HP ticks applied.
    pub fn heal_residual_ambulance_heals(&self) -> u32 {
        self.heal_residual_ambulance_heals
    }

    /// Residual HealPad SeekingHealing HP ticks applied.
    pub fn heal_residual_heal_pad_heals(&self) -> u32 {
        self.heal_residual_heal_pad_heals
    }

    /// Record an ambulance radius AutoHeal infantry HP tick.
    pub fn record_ambulance_residual_heal(&mut self) {
        self.heal_residual_ambulance_heals = self.heal_residual_ambulance_heals.saturating_add(1);
    }

    /// Record a HealPad SeekingHealing HP tick.
    pub fn record_heal_pad_residual_heal(&mut self) {
        self.heal_residual_heal_pad_heals = self.heal_residual_heal_pad_heals.saturating_add(1);
    }

    /// Residual ambulance infantry heal honesty: at least one radius AutoHeal tick.
    /// Fail-closed: not full sole-benefactor / vehicle AutoHeal ModuleTag_23 parity.
    pub fn honesty_ambulance_heal_ok(&self) -> bool {
        self.heal_residual_ambulance_heals > 0
    }

    /// Residual HealPad infantry heal honesty: at least one SeekingHealing tick.
    pub fn honesty_heal_pad_ok(&self) -> bool {
        self.heal_residual_heal_pad_heals > 0
    }

    /// Combined host infantry heal residual honesty (ambulance radius or HealPad).
    pub fn honesty_heal_ok(&self) -> bool {
        self.honesty_ambulance_heal_ok() || self.honesty_heal_pad_ok()
    }

    /// Host propaganda tower residual heal honesty ticks.
    pub fn propaganda_residual_heals(&self) -> u32 {
        self.propaganda_residual_heals
    }

    /// Host propaganda tower residual buff honesty ticks.
    pub fn propaganda_residual_buffs(&self) -> u32 {
        self.propaganda_residual_buffs
    }

    pub(in super::super) fn record_propaganda_residual_heal(&mut self) {
        self.propaganda_residual_heals = self.propaganda_residual_heals.saturating_add(1);
    }

    pub(in super::super) fn record_propaganda_residual_buff(&mut self) {
        self.propaganda_residual_buffs = self.propaganda_residual_buffs.saturating_add(1);
    }

    /// Residual honesty: speaker/propaganda tower healed at least one unit.
    pub fn honesty_propaganda_heal_ok(&self) -> bool {
        self.propaganda_residual_heals > 0
    }

    /// Residual honesty: speaker/propaganda tower granted ENTHUSIASTIC/SUBLIMINAL buff.
    pub fn honesty_propaganda_buff_ok(&self) -> bool {
        self.propaganda_residual_buffs > 0
    }

    /// Combined host propaganda tower residual honesty (heal or buff).
    pub fn honesty_propaganda_ok(&self) -> bool {
        self.honesty_propaganda_heal_ok() || self.honesty_propaganda_buff_ok()
    }

    /// Host ECM tank residual jam honesty ticks (DISABLED_SUBDUED grants).
    pub fn ecm_residual_jams(&self) -> u32 {
        self.ecm_residual_jams
    }

    pub(in super::super) fn record_ecm_residual_jam(&mut self) {
        self.ecm_residual_jams = self.ecm_residual_jams.saturating_add(1);
    }

    /// Residual honesty: ECM tank / jammer jammed enemy weapons at least once.
    pub fn honesty_ecm_jam_ok(&self) -> bool {
        self.ecm_residual_jams > 0
            || self.ecm_missiles_jammed > 0
            || self.ecm_laser_beams_spawned > 0
    }

    /// Residual honesty: ECMDisableStream laser spawned at least once.
    pub fn honesty_ecm_laser_ok(&self) -> bool {
        self.ecm_laser_beams_spawned > 0
    }

    /// Host Microwave Tank residual registry (disable structure honesty).
    pub fn microwave_residual(&self) -> &crate::game_logic::host_microwave::HostMicrowaveRegistry {
        &self.microwaves
    }

    /// Residual honesty: Microwave tank disabled an enemy structure at least once.
    pub fn honesty_microwave_disable_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
    }

    /// Residual honesty: MicrowaveDisableStream laser spawned at least once.
    pub fn honesty_microwave_laser_ok(&self) -> bool {
        self.microwaves.honesty_laser_ok()
    }

    /// Residual honesty: emitter MICROWAVE field damaged at least once.
    pub fn honesty_microwave_emitter_ok(&self) -> bool {
        self.microwaves.honesty_emitter_ok()
    }

    /// Combined host path honesty for Microwave residual (disable).
    /// Garrison clear honesty is tracked separately via `honesty_kill_garrisoned_ok`.
    pub fn honesty_microwave_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
            || self.microwaves.honesty_laser_ok()
            || self.microwaves.honesty_emitter_ok()
    }

    /// Host EMP Pulse residual registry (activate + honesty).
    pub fn emp_pulses(&self) -> &crate::game_logic::host_emp_pulse::HostEmpPulseRegistry {
        &self.emp_pulses
    }

    /// Residual honesty: EmpPulse activated at least once.
    pub fn honesty_emp_pulse_activate_ok(&self) -> bool {
        self.emp_pulses.honesty_activate_ok()
    }

    /// Residual honesty: EmpPulse applied DISABLED_EMP at least once.
    pub fn honesty_emp_pulse_disable_ok(&self) -> bool {
        self.emp_pulses.honesty_disable_ok()
    }

    /// Combined host path honesty for EmpPulse residual.
    pub fn honesty_emp_pulse_ok(&self) -> bool {
        self.emp_pulses.honesty_host_path_ok()
    }

    /// Residual honesty: Baikonur launch door and/or detonation recorded.

    pub fn honesty_defector_ok(&self) -> bool {
        self.defector_special.honesty_ok()
    }

    /// C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
    /// ActionManager.cpp:1696-1710 rejects STRUCTURE and non-ENEMIES;
    /// Object.cpp:6111-6220 `defect` after those guards.
    pub fn activate_defector(&mut self, caster_id: ObjectId, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_defector_special_power::{
            resolve_voice_defect, DEFECTOR_DETECTION_FRAMES, DEFECTOR_TIMER_TICK_AUDIO,
        };
        if caster_id == victim_id {
            return false;
        }
        let Some(caster) = self.objects.get(&caster_id) else {
            return false;
        };
        if caster.is_disabled() {
            return false;
        }
        let caster_team = caster.team;
        if caster_team == Team::Neutral {
            return false;
        }
        let caster_owner = self.player_owner_for_host_object(caster);
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if !victim.is_alive() {
            return false;
        }
        if victim.is_kind_of(KindOf::Structure) {
            return false;
        }
        // C++ relationship ENEMIES only (neutral / same-team are worthless).
        if victim.team == caster_team || victim.team == Team::Neutral {
            return false;
        }
        if victim.contained_by.is_some() {
            return false;
        }
        if victim.status.under_construction
            || victim.construction_percent + 0.001 < 1.0
            || victim.status.sold
        {
            return false;
        }
        let old_team = victim.team;
        let old_owner = self.player_owner_for_host_object(victim);
        let victim_pos = victim.get_position();
        let victim_template = victim.template_name.clone();
        let frames = DEFECTOR_DETECTION_FRAMES;
        let now = self.frame;

        // C++ Object::defect before switch: refund production, radar ping.
        self.cancel_all_production(victim_id);
        let old_playable = old_owner
            .map(|id| self.player_is_playable_side(id))
            .unwrap_or(false);
        let new_playable = caster_owner
            .map(|id| self.player_is_playable_side(id))
            .unwrap_or(caster_team != Team::Neutral);
        if old_playable && new_playable {
            self.try_infiltration_event(victim_id);
        }

        let Some(victim) = self.objects.get_mut(&victim_id) else {
            return false;
        };
        victim.set_team_and_owner(caster_team, caster_owner);
        victim.begin_undetected_defection(now, frames, true);

        // C++ after switch: handlePartitionCellMaintenance + aiIdle.
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            victim.stop_moving();
            victim.set_status_moving(false);
            victim.set_status_attacking(false);
            victim.set_target(None);
            victim.set_ai_state(AIState::Idle);
            victim.flash_as_selected();
        }
        self.stop_attack_decision_aware(victim_id);
        self.clear_target_decision_aware(victim_id);

        // C++ `*getTemplate()->getVoiceDefect()` + defector timer tick.
        // Missing VoiceDefect is an empty AudioEventRTS (silent), never the slot token.
        if let Some(event) = resolve_voice_defect(&victim_template) {
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(victim_id)
                    .with_position(victim_pos)
                    .with_priority(180),
            );
        }
        self.queue_audio_event(
            AudioEventRequest::new(DEFECTOR_TIMER_TICK_AUDIO)
                .with_object(victim_id)
                .with_position(victim_pos)
                .with_priority(160),
        );

        // C++ kickOutOnCapture removeAllContained (tunnels/caves skip).
        self.on_capture_kick_passengers(victim_id, old_team, caster_team);

        // C++ ParkingPlaceBehavior::defectAllParkedUnits.
        self.defect_all_parked_units(victim_id);

        // C++ world walk: KINDOF_MINE whose producer is this object setTeam.
        let mine_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_kind_of(KindOf::Mine) && o.producer_id == Some(victim_id))
            .map(|(id, _)| *id)
            .collect();
        for mine_id in mine_ids {
            if let Some(mine) = self.objects.get_mut(&mine_id) {
                mine.set_team_and_owner(caster_team, caster_owner);
            }
        }

        self.defector_special.record(victim_id.0 as u32, frames);
        true
    }

    /// C++ SpecialPowerModule ctor path: StartsPaused → pauseCountdown(TRUE).

    /// C++ SupplyWarehouseCreate::onCreate residual.

    /// C++ SpecialPowerCompletionDie::onDie → notifyOfCompletedSpecialPower residual.
    pub(crate) fn maybe_notify_special_power_completion(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let Some(ref data) = obj.special_power_completion else {
            return;
        };
        if !data.creator_set {
            return;
        }
        let power = data.special_power_name.clone();
        let creator = data.creator_id;
        let team = obj.team;
        let player_id = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.id)
            .unwrap_or(0);
        crate::game_logic::script_events::push_event(
            crate::game_logic::script_events::ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name: power.clone(),
                creator_id: creator,
            },
        );
        self.special_power_completion_log
            .record_notify(&power, creator);
    }

    /// C++ PowerPlantUpdate::extendRods(true) residual — start rod animation timer.
    pub fn begin_power_plant_rods_extend(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        use crate::game_logic::host_special_power_completion_die::rods_extend_frames_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if obj.power_plant_rods_extended && obj.power_plant_rods_done_frame == 0 {
            // Already fully extended.
            return false;
        }
        if obj.power_plant_rods_done_frame > 0 {
            // Already extending.
            return false;
        }
        // Parsed `PowerPlantUpdate::RodsExtendTime` owns this value whenever
        // this object crossed the Object INI metadata boundary.  Keep the
        // older residual helper only for hand-authored templates used by
        // unrelated existing PowerPlantUpgrade paths.
        let frames = obj
            .thing
            .template
            .power_plant_update
            .map(|metadata| metadata.rods_extend_time_frames)
            .unwrap_or_else(|| rods_extend_frames_for_template(&obj.template_name));
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits |= 1u128 << bit;
        }
        // Clear upgraded while animating.
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_done_frame = self.frame.saturating_add(frames.max(1));
        obj.power_plant_rods_extended = true;
        self.special_power_completion_log.record_rods_start();
        true
    }

    /// C++ `PowerPlantUpdate::extendRods(false)` — retract immediately and
    /// clear both animation conditions.  Overcharge calls this only when the
    /// parsed object actually exposes the PowerPlantUpdate interface.
    pub fn retract_power_plant_rods(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_extended = false;
        obj.power_plant_rods_done_frame = 0;
        true
    }

    /// C++ PowerPlantUpdate::update residual — finish rod extend.
    pub fn update_power_plant_rods(&mut self) {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let now = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.power_plant_rods_done_frame > 0 && o.power_plant_rods_done_frame <= now
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                    obj.model_condition_bits |= 1u128 << bit;
                }
                obj.power_plant_rods_done_frame = 0;
                self.special_power_completion_log.record_rods_complete();
            }
        }
    }

    pub(in super::super) fn init_supply_warehouse_create(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_structure_economy_residual::starting_supplies_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        // Retail `SupplyWarehouseDockUpdate::StartingBoxes` is authoritative.
        // Retain the legacy bootstrap table only for hand-authored templates
        // that have no parsed Behavior metadata; it never grants Dock ability.
        let supplies = if obj.thing.template.dock_kind
            == crate::game_logic::DockKind::SupplyWarehouse
        {
            obj.thing.template.dock_starting_boxes.map(|boxes| {
                boxes.saturating_mul(
                    crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32,
                )
            })
        } else {
            starting_supplies_for_template(&obj.template_name)
        };
        let has_warehouse_create = obj.thing.template.has_supply_warehouse_create
            || obj.thing.template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse;
        let had_supplies = supplies.is_some();
        if let Some(supplies) = supplies {
            // Only seed if empty (map may already set amount).
            if obj.stored_resources.supplies == 0 {
                obj.set_stored_supplies(supplies);
            }
        }
        if !has_warehouse_create && !had_supplies {
            return;
        }
        drop(obj);
        if has_warehouse_create || had_supplies {
            self.supply_create_warehouse_registers =
                self.supply_create_warehouse_registers.saturating_add(1);
        }
        if has_warehouse_create {
            for player in self.players.values_mut() {
                player.add_supply_warehouse(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_warehouse(object_id.0);
                }
            }
        }
    }

    /// C++ SupplyCenterCreate::onBuildComplete residual.
    pub(in super::super) fn on_supply_center_build_complete(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_upgrades::is_supply_center_template;
        let is_supply_center = self.objects.get(&object_id).is_some_and(|obj| {
            obj.thing.template.has_supply_center_create
                || obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || is_supply_center_template(&obj.template_name)
        });
        if is_supply_center {
            self.supply_create_center_registers =
                self.supply_create_center_registers.saturating_add(1);
            // C++ walks every player ResourceGatheringManager::addSupplyCenter.
            for player in self.players.values_mut() {
                player.add_supply_center(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_center(object_id.0);
                }
            }
            // C++ SupplyCenter/Stash SpawnBehavior ModuleTag_12 only becomes
            // eligible after UNDER_CONSTRUCTION clears.  It creates the free
            // starter collector outside the paid ProductionUpdate queue.
            let _ = self.spawn_supply_center_one_shot_collector(object_id);
        }
    }

    /// C++ GenerateMinefieldBehavior::upgradeImplementation + EMP swap residual.
    pub(in super::super) fn place_structure_minefield_for_upgrade(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> u32 {
        self.apply_structure_minefield_upgrade(object_id, upgrade)
    }

    pub(in super::super) fn init_starts_paused_special_powers(&mut self, object_id: ObjectId) {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_upgrade_module_residuals::power_starts_paused;
        // C++ SpecialPowerModule starts the authored ReloadTime on creation
        // before applying StartsPaused.  HDB is not covered by the old
        // handwritten special-power table, so retain the paired Object INI
        // metadata here rather than falling through to a Hacker name.
        let hacker_disable = self
            .objects
            .get(&object_id)
            .and_then(|obj| obj.thing.template.hacker_disable_building.clone());
        if let Some(metadata) = hacker_disable {
            let owner_id = self
                .objects
                .get(&object_id)
                .and_then(|obj| self.player_owner_for_host_object(obj));
            if metadata.shared_n_sync {
                // SharedNSync belongs to the exact controller, never the
                // first player selected by faction.  Missing/stale ownership
                // remains unavailable through the typed readiness gate.
                if let Some(owner_id) = owner_id {
                    if let Some(player) = self.get_player_mut(owner_id) {
                        player.reset_shared_special_power_timer(
                            &P::HackerDisableBuilding,
                            metadata.reload_time_frames as f32 / 30.0,
                        );
                    }
                }
            } else if let Some(object) = self.objects.get_mut(&object_id) {
                if !object.status.under_construction {
                    object.start_power_recharge_with_frames(
                        &P::HackerDisableBuilding,
                        metadata.reload_time_frames,
                    );
                }
            }
            if metadata.starts_paused {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.pause_special_power_countdown(&P::HackerDisableBuilding, true);
                }
            }
        }
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if obj.thing.template.capture_starts_paused {
            if let Some(power) = obj.thing.template.capture_power.special_power_type() {
                obj.pause_special_power_countdown(&power, true);
            }
        }
        let name = obj.template_name.to_ascii_lowercase();
        let candidates = [P::RadarScan, P::HelixNapalmBomb];
        for power in candidates {
            if !power_starts_paused(&power) {
                continue;
            }
            let relevant = match power {
                P::RadarScan => name.contains("radarvan") || name.contains("radar_van"),
                P::HelixNapalmBomb => name.contains("helix"),
                _ => false,
            };
            if relevant {
                obj.pause_special_power_countdown(&power, true);
            }
        }
    }

    /// C++ `ThingTemplate::calcCostToBuild` player modifier path.
    ///
    /// The exact `PlayerTemplate::ProductionCostChange` comes first, then the
    /// independently stacked KindOf upgrade modifiers.  The old host helper
    /// returned early for an unclassified template, which incorrectly skipped
    /// a General's exact-name discount even though C++ applies it before the
    /// KindOf query.
    pub fn modified_build_cost_supplies(
        &self,
        player_id: u32,
        template_name: &str,
        base_supplies: u32,
    ) -> u32 {
        use crate::game_logic::host_upgrade_module_residuals::{
            apply_production_cost_factor, kindof_cost_tokens,
        };
        let Some(player) = self.players.get(&player_id) else {
            return base_supplies;
        };
        let (is_vehicle, is_infantry, is_aircraft, is_structure) = self
            .templates
            .get(template_name)
            .map(|t| {
                (
                    t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                    t.is_kind_of(crate::game_logic::KindOf::Infantry),
                    t.is_kind_of(crate::game_logic::KindOf::Aircraft),
                    t.is_kind_of(crate::game_logic::KindOf::Structure),
                )
            })
            .unwrap_or((false, false, false, false));
        let tokens = kindof_cost_tokens(is_vehicle, is_infantry, is_aircraft, is_structure);
        let kindof_factor = player.production_cost_factor(&tokens);
        let template_factor = self.player_template_production_cost_factor(player_id, template_name);
        let handicap = player.handicap_build_cost_multiplier(is_structure);
        apply_production_cost_factor(
            base_supplies,
            template_factor * kindof_factor * handicap,
        )
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored pre-power frame count.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Keep this integer form available to both queued production and
    /// dozer construction, before either path applies the low-power penalty.
    pub(crate) fn cpp_build_time_frames_from_factor(base_seconds: f32, factor: f32) -> u32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

        if !base_seconds.is_finite() || !factor.is_finite() {
            // Invalid INI values must not manufacture a near-instant unit.
            return u32::MAX;
        }

        let base_frames = (base_seconds * LOGIC_FRAMES_PER_SECOND)
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32;
        ((base_frames as f32) * factor.max(0.0))
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32
    }

    /// Encode C++ `ThingTemplate::calcTimeToBuild`'s authored pre-power frame
    /// count in Main's legacy seconds carrier.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Main's queue stores seconds but its existing completion code
    /// recovers an integer frame count before applying the low-power penalty.
    /// Preserve that ordering by encoding the already-truncated pre-power
    /// frame count just above its lower frame boundary.  A direct
    /// `frames as f32 / 30.0` can round below that boundary (for example frame
    /// 63), causing the downstream `.trunc()` to lose a frame.
    pub(crate) fn cpp_build_time_seconds_from_factor(base_seconds: f32, factor: f32) -> f32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
        const FRAME_ENCODING_FRACTION: f32 = 0.25;
        let authored_frames = Self::cpp_build_time_frames_from_factor(base_seconds, factor);
        if authored_frames == 0 {
            return 0.0;
        }

        (authored_frames as f32 + FRAME_ENCODING_FRACTION) / LOGIC_FRAMES_PER_SECOND
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored PlayerTemplate stage.
    ///
    /// This returns a seconds carrier for `ProductionItem`; its existing
    /// logic-frame completion path then applies the C++ low-energy penalty.
    pub(crate) fn modified_build_time_seconds(
        &self,
        player_id: u32,
        template_name: &str,
        base_seconds: f32,
    ) -> f32 {
        let is_structure = self
            .templates
            .get(template_name)
            .map(|t| t.is_kind_of(crate::game_logic::KindOf::Structure))
            .unwrap_or(false);
        let handicap = self
            .players
            .get(&player_id)
            .map(|p| p.handicap_build_time_multiplier(is_structure))
            .unwrap_or(1.0);
        let factor =
            self.player_template_production_time_factor(player_id, template_name) * handicap;
        Self::cpp_build_time_seconds_from_factor(base_seconds, factor)
    }

    pub fn honesty_baikonur_ok(&self) -> bool {
        self.baikonur_launches.honesty_host_path_ok()
    }

    pub fn baikonur_launches(
        &self,
    ) -> &crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry {
        &self.baikonur_launches
    }

    /// C++ BaikonurLaunchPower::doSpecialPower residual — DOOR_1_OPENING on tower.
    pub fn activate_baikonur_launch_door(&mut self, source_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
        let Some(obj) = self.objects.get_mut(&source_id) else {
            return false;
        };
        if obj.is_disabled() {
            return false;
        }
        let bit = door_1_opening_model_bit();
        obj.model_condition_bits |= 1u128 << bit;
        obj.refresh_model_condition_bits();
        self.baikonur_launches.record_launch_door();
        true
    }

    /// C++ BaikonurLaunchPower::doSpecialPowerAtLocation residual —
    /// spawn BaikonurRocketDetonation + NeutronMissileSlowDeath multi-blast.
    pub fn activate_baikonur_detonation(
        &mut self,
        source_id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        use crate::game_logic::host_baikonur_launch::{
            BAIKONUR_DETONATION_OBJECT, BAIKONUR_NUKE_FX,
        };
        let Some(src) = self.objects.get(&source_id) else {
            return false;
        };
        if src.is_disabled() {
            return false;
        }
        let team = src.team;
        // Ensure detonation template exists residual.
        if !self.templates.contains_key(BAIKONUR_DETONATION_OBJECT) {
            let mut t = crate::game_logic::ThingTemplate::new(BAIKONUR_DETONATION_OBJECT);
            t.set_health(1.0);
            t.add_kind_of(crate::game_logic::KindOf::Immobile);
            self.templates
                .insert(BAIKONUR_DETONATION_OBJECT.to_string(), t);
        }
        let det_id = match self.create_object(BAIKONUR_DETONATION_OBJECT, team, location) {
            Some(id) => id,
            None => return false,
        };
        // Arm Neutron multi-blast residual at detonation (same as nuke impact).
        let _ = self
            .special_power_strikes
            .spawn_neutron_slow_death_field(det_id, team, location, self.frame, 0);
        // Presentation FX residual name on detonation object.
        if let Some(d) = self.objects.get_mut(&det_id) {
            d.pending_death_fx = Some(BAIKONUR_NUKE_FX.to_string());
            // Lifetime 0 residual — mark for quick completion after blasts.
            d.ensure_lifetime_update(self.frame);
        }
        self.baikonur_launches
            .record_detonation(location.x, location.z);
        // Queue audio residual.
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new("BaikonurRocketDetonation")
                .with_object(det_id)
                .with_position(location)
                .with_priority(200),
        );
        true
    }

    /// Activate EmpPulse residual: temporarily disable vehicles/structures in radius.
    ///
    /// Matches retail SuperweaponEMPPulse → EMPPulseEffectSpheroid EMPUpdate:
    /// - Radius residual 200 (RadiusCursorRadius / default EffectRadius)
    /// - DisabledDuration 30000 ms → 900 logic frames (DISABLED_EMP)
    /// - Vehicles + faction structures disabled; airborne aircraft killed residual
    ///
    /// Fail-closed: not full OCL bomb / spheroid drawable / spark particle path.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_emp_pulse(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        // C++ SUPERWEAPON_EMPPulse DeliverPayload residual: cargo plane + bomb first.
        if let Some(cid) = caster_id {
            if self
                .spawn_emp_pulse_flight(cid, location, player_id)
                .is_some()
            {
                return true;
            }
        }
        self.apply_emp_pulse_at(player_id, location, caster_id)
    }

    /// Apply EMP disable field residual at location (bomb impact / fail-open path).
    ///
    /// C++ EMPUpdate ctor sets `m_tintEnvPlayFrame = now + StartFadeTime` and
    /// only calls `doDisableAttack` on that exact frame. Spawn the spheroid now;
    /// disable waits StartFadeTime (9 frames).
    pub fn apply_emp_pulse_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_emp_pulse::EMP_PULSE_ACTIVATE_AUDIO;

        let frame = self.frame;
        let spheroid_id = if let Some(pid) = caster_id {
            self.spawn_emp_pulse_spheroid(location, pid)
        } else {
            self.objects
                .keys()
                .next()
                .copied()
                .and_then(|pid| self.spawn_emp_pulse_spheroid(location, pid))
        };
        if let Some(sid) = spheroid_id {
            self.emp_pulses
                .begin_spheroid(sid, player_id, location, caster_id, frame);
        }

        self.queue_audio_event(
            AudioEventRequest::new(EMP_PULSE_ACTIVATE_AUDIO)
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

    /// C++ EMPUpdate::doDisableAttack residual (FROM_BOUNDINGSPHERE_3D).
    /// `curVictim != object` skips the EMPPulseEffectSpheroid, not the caster.
    /// Pulse spheroid DoesNotAffectMyOwnBuildings=No — own buildings disable.
    pub fn apply_emp_pulse_disable_field_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_emp_pulse::{
            in_emp_pulse_radius_from_bounding_sphere_3d, is_emp_hardened_name,
            is_legal_emp_disable_target, leftover_emp_bounding_sphere_radius,
            should_emp_kill_airborne, should_emp_skip_hardened_airborne, HostEmpPulse,
            EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
        };

        let frame = self.frame;
        let until = frame.saturating_add(EMP_PULSE_DISABLED_DURATION_FRAMES);

        let candidates: Vec<(
            ObjectId,
            bool,
            bool,
            bool,
            bool,
            bool,
            bool,
            bool,
        )> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                // C++ EMPUpdate.cpp:192 — skip the spheroid (`object`), not caster_id.
                if obj.emp_pulse_spheroid {
                    return None;
                }
                let pos = obj.get_position();
                let sphere = leftover_emp_bounding_sphere_radius(
                    obj.thing.geometry.radius,
                    obj.thing.geometry.bounds_min,
                    obj.thing.geometry.bounds_max,
                    obj.selection_radius,
                );
                if !in_emp_pulse_radius_from_bounding_sphere_3d(
                    location,
                    pos,
                    sphere,
                    HOST_EMP_PULSE_RADIUS,
                ) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let is_faction_structure = is_structure && obj.is_faction_structure();
                let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                let is_airborne = obj.status.airborne_target;
                let is_spawns = obj
                    .template_name
                    .to_ascii_lowercase()
                    .contains("spawnsaretheweapons")
                    || obj.template_name.to_ascii_lowercase().contains("stinger");
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let emp_hardened = is_emp_hardened_name(&obj.template_name);
                Some((
                    *id,
                    is_vehicle,
                    is_faction_structure,
                    is_aircraft,
                    is_airborne,
                    is_spawns,
                    under_construction,
                    emp_hardened,
                ))
            })
            .collect();

        let mut disables: u32 = 0;
        let mut airborne_kills: u32 = 0;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut spark_ids: Vec<ObjectId> = Vec::new();

        for (
            id,
            is_vehicle,
            is_faction_structure,
            is_aircraft,
            is_airborne,
            is_spawns,
            under_construction,
            emp_hardened,
        ) in candidates
        {
            if should_emp_kill_airborne(is_aircraft, is_airborne, emp_hardened) {
                destroy_ids.push(id);
                airborne_kills = airborne_kills.saturating_add(1);
                continue;
            }
            // C++ EMPUpdate.cpp:240-241 — EMP_HARDENED airborne continue.
            if should_emp_skip_hardened_airborne(is_aircraft, is_airborne, emp_hardened) {
                continue;
            }

            if !is_legal_emp_disable_target(
                is_vehicle,
                is_faction_structure,
                is_spawns,
                true,
                under_construction,
                emp_hardened,
            ) {
                continue;
            }

            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            target.apply_disabled_emp(until);
            disables = disables.saturating_add(1);
            spark_ids.push(id);
        }

        for id in destroy_ids {
            let killer_team = caster_id
                .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
                .unwrap_or(Team::Neutral);
            self.mark_object_for_destruction(id, Some(killer_team));
        }
        // C++ doDisableAttack EMPSparks on disabled victims (not airborne kills).
        for vid in spark_ids {
            self.spawn_emp_sparks_on_victim(vid, EMP_PULSE_DISABLED_DURATION_FRAMES);
        }


        let pulse_id = self.emp_pulses.alloc_id();
        self.emp_pulses.record_activation(HostEmpPulse {
            id: pulse_id,
            player_id,
            location,
            radius: HOST_EMP_PULSE_RADIUS,
            activate_frame: frame,
            disable_until_frame: until,
            caster_id,
            disables,
            airborne_kills,
        });
        true
    }

    /// Fire leftover EMPUpdate doDisableAttack on StartFadeTime frames.
    pub fn apply_due_emp_pulse_disables(&mut self) {
        use crate::game_logic::host_emp_pulse::EMP_SPHEROID_GEOMETRY_RADIUS;

        let now = self.frame;
        self.emp_pulses.tick_spheroids(now);
        let visual: Vec<(ObjectId, f32)> = self
            .emp_pulses
            .spheroids()
            .iter()
            .map(|s| (s.id, s.current_scale))
            .collect();
        for (id, scale) in visual {
            if let Some(o) = self.objects.get_mut(&id) {
                if o.emp_pulse_spheroid {
                    o.thing.geometry.radius = EMP_SPHEROID_GEOMETRY_RADIUS * scale;
                    o.visual_draw_state_revision = o.visual_draw_state_revision.wrapping_add(1);
                }
            }
        }
        let due = self.emp_pulses.due_disable_spheroids(now);
        for sph in due {
            self.emp_pulses.mark_disable_applied(sph.id);
            let _ = self.apply_emp_pulse_disable_field_at(
                sph.player_id,
                sph.location,
                sph.caster_id,
            );
        }
    }

    /// Host China Frenzy ("Rage") residual registry (activate + honesty).
    pub fn frenzies(&self) -> &crate::game_logic::host_frenzy::HostFrenzyRegistry {
        &self.frenzies
    }

    /// Residual honesty: Frenzy activated at least once.
    pub fn honesty_frenzy_activate_ok(&self) -> bool {
        self.frenzies.honesty_activate_ok()
    }

    /// Residual honesty: Frenzy applied attack buff at least once.
    pub fn honesty_frenzy_buff_ok(&self) -> bool {
        self.frenzies.honesty_buff_ok()
    }

    /// Combined host path honesty for Frenzy / Rage residual.
    pub fn honesty_frenzy_ok(&self) -> bool {
        self.frenzies.honesty_host_path_ok()
    }

    /// Host USA Strategy Center battle-plan residual registry (select + honesty).
    pub fn battle_plans(&self) -> &crate::game_logic::host_strategy_center::HostBattlePlanRegistry {
        &self.battle_plans
    }

    /// C++ `Player::xfer` (`Player.cpp:4480-4507`) restores `m_battlePlanBonuses`.
    pub fn restore_battle_plans(
        &mut self,
        registry: crate::game_logic::host_strategy_center::HostBattlePlanRegistry,
    ) {
        self.battle_plans = registry;
    }


    /// Residual honesty: Strategy Center battle plan selected at least once.
    pub fn honesty_battle_plan_select_ok(&self) -> bool {
        self.battle_plans.honesty_select_ok()
    }

    /// Residual honesty: battle plan applied army residual buff at least once.
    pub fn honesty_battle_plan_buff_ok(&self) -> bool {
        self.battle_plans.honesty_buff_ok()
    }

    /// Residual honesty: BattlePlanChangeParalyze residual applied at least once.
    pub fn honesty_battle_plan_paralyze_ok(&self) -> bool {
        self.battle_plans.honesty_paralyze_ok()
    }

    /// Combined host path honesty for Strategy Center battle-plan residual.
    pub fn honesty_battle_plan_ok(&self) -> bool {
        self.battle_plans.honesty_host_path_ok()
    }

    /// Residual honesty: Bombardment turret StrategyCenterGun fired.
    pub fn honesty_battle_plan_turret_fire_ok(&self) -> bool {
        self.battle_plans.honesty_turret_fire_ok()
    }

    /// Residual honesty: StealthDetectorUpdate enabled (SearchAndDestroy residual).
    pub fn honesty_battle_plan_stealth_detector_ok(&self) -> bool {
        self.battle_plans.honesty_stealth_detector_ok()
    }

    /// Residual honesty: pack/unpack door residual started.
    pub fn honesty_battle_plan_door_ok(&self) -> bool {
        self.battle_plans.honesty_door_residual_ok()
    }

    /// Residual honesty: door residual reached ACTIVE / WAITING_TO_CLOSE.
    pub fn honesty_battle_plan_door_active_ok(&self) -> bool {
        self.battle_plans.honesty_door_active_ok()
    }

    /// Residual honesty: delayed setBattlePlan applied after unpack ACTIVE.
    pub fn honesty_battle_plan_delayed_active_ok(&self) -> bool {
        self.battle_plans.honesty_delayed_active_apply_ok()
    }

    /// Residual honesty: setBattlePlan(NONE) pack-clear residual fired.
    pub fn honesty_battle_plan_pack_clear_ok(&self) -> bool {
        self.battle_plans.honesty_pack_clear_ok()
    }

    /// Residual honesty: Bombardment turret recenter residual before pack.
    pub fn honesty_battle_plan_turret_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_recenter_ok()
    }

    /// Residual honesty: Strategy Center turret pitch/yaw left natural (aim residual).
    pub fn honesty_strategy_center_turret_aim_ok(&self) -> bool {
        self.objects.values().any(|o| {
            crate::game_logic::host_strategy_center::is_strategy_center_template(&o.template_name)
                && !crate::game_logic::host_strategy_center::turret_angles_are_natural(
                    o.turret_angle_deg,
                    o.turret_pitch_deg,
                )
        })
    }

    /// Residual honesty: TurretAI idle-scan residual started (Bombardment ACTIVE).
    pub fn honesty_strategy_center_turret_idle_scan_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_scan_ok()
    }

    /// Residual honesty: TurretAI HoldTurret residual started (after idle-scan).
    pub fn honesty_strategy_center_turret_hold_ok(&self) -> bool {
        self.battle_plans.honesty_turret_hold_ok()
    }

    /// Residual honesty: TurretAI idle-recenter residual completed (after Hold).
    pub fn honesty_strategy_center_turret_idle_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_recenter_ok()
    }

    /// Tick TurretAI idle mood-target residual for Bombardment ACTIVE Strategy Centers.
    ///
    /// C++ `TurretAI::friend_checkForIdleMoodTarget` residual:
    /// - When idle, acquire nearest legal enemy in StrategyCenterGun range band
    /// - Aim pitch/yaw at target (FirePitch **45**), flag `m_targetWasSetByIdleMood`
    /// - While held: re-aim each frame; clear when dead / OOR / illegal (team/air/UC)
    /// - Mood matrix Sleep → IgnoreAll (no acquire); Passive → WaitForAttack
    ///   (only last_damage_source residual); Normal/Alert/Aggressive → free
    /// - Fire residual ownership: bombardment fire clears mood flag if it engages
    ///   a different target (see `try_strategy_center_bombardment_turret_fire`)
    pub(in super::super) fn tick_strategy_center_turret_mood_target(&mut self) {
        use crate::game_logic::host_strategy_center::{
            is_strategy_center_template, strategy_center_gun_in_range,
            strategy_center_mood_target_eligible_with_attitude,
            strategy_center_mood_target_enemy_legal_with_vision,
            strategy_center_mood_target_in_vision,
            strategy_center_mood_target_should_clear_with_vision,
            strategy_center_mood_vision_range, strategy_center_turret_aim_at, HostAiAttitude,
            HostBattlePlan, HostBattlePlanTransition,
        };

        // Bombardment ACTIVE centers.
        let centers: Vec<ObjectId> = self
            .battle_plans
            .door_states()
            .iter()
            .filter(|s| {
                s.status == HostBattlePlanTransition::Active
                    && s.door_plan == Some(HostBattlePlan::Bombardment)
                    && !s.centering_turret
            })
            .map(|s| s.center_id)
            .collect();

        let mut acquires = 0u32;
        let mut clears = 0u32;
        for cid in centers {
            let Some(obj) = self.objects.get(&cid) else {
                continue;
            };
            if !obj.is_alive() || !is_strategy_center_template(&obj.template_name) {
                continue;
            }
            if obj.weapon.is_none() {
                continue;
            }
            let team = obj.team;
            let fire_pos = obj.get_position();
            let has_mood = obj.turret_mood_target;
            let attitude = HostAiAttitude::from_i8(obj.ai_attitude);
            let last_dmg = obj.last_damage_source;
            // Partition / AI vision residual: VisionRange **400**.
            // Bombardment ACTIVE path: S&D sight scalar does not apply (plans
            // are mutually exclusive). Host residual still uses the vision
            // filter helper so reduced-vision / S&D matrix stays host-testable.
            let vision_range = strategy_center_mood_vision_range(false);
            // "Busy" for acquire: only non-mood attacking (pack recenter / explicit
            // non-mood attack). Mood-set Attacking is the hold state, not busy.
            let busy_non_mood = !has_mood
                && (obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    ));

            // Hold / clear / re-aim mood target residual.
            if has_mood {
                let tgt = obj.target;
                let mut clear = tgt.is_none();
                let mut aim_xz: Option<(f32, f32)> = None;
                if let Some(tid) = tgt {
                    if let Some(t) = self.objects.get(&tid) {
                        let tp = t.get_position();
                        let dx = tp.x - fire_pos.x;
                        let dz = tp.z - fire_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
                        let legal = strategy_center_mood_target_enemy_legal_with_vision(
                            t.is_alive(),
                            t.team == team,
                            t.team == Team::Neutral,
                            t.status.under_construction,
                            is_air,
                            dist,
                            vision_range,
                        );
                        let in_range = strategy_center_gun_in_range(dist);
                        let in_vision = strategy_center_mood_target_in_vision(dist, vision_range);
                        clear = strategy_center_mood_target_should_clear_with_vision(
                            true,
                            t.is_alive(),
                            in_range,
                            in_vision,
                        ) || !legal;
                        if !clear {
                            aim_xz = Some((tp.x, tp.z));
                        }
                    } else {
                        clear = true;
                    }
                }
                if clear {
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_mood_target = false;
                        o.set_status_attacking(false);
                        o.target = None;
                        if matches!(o.ai_state, AIState::Attacking) {
                            o.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(cid, 0);
                            }
                        }
                    }
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_stop_attack(cid);
                    }
                    clears = clears.saturating_add(1);
                } else if let Some((tx, tz)) = aim_xz {
                    // C++ AIM continuous aim residual while mood target held.
                    let (aim_a, aim_p) =
                        strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_angle_deg = aim_a;
                        o.record_host_turret();
                        o.turret_pitch_deg = aim_p;
                        o.record_host_turret();
                        o.set_ai_state(AIState::Attacking);
                        o.set_status_attacking(true);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                        }
                    }
                }
                continue; // no re-acquire this frame while mood flag set
            }

            // Passive WaitForAttack: only retaliate vs last_damage_source residual.
            let passive_last = last_dmg.is_some();
            if !strategy_center_mood_target_eligible_with_attitude(
                true,
                true,
                busy_non_mood,
                has_mood,
                attitude,
                passive_last,
            ) {
                continue;
            }

            // Find residual mood target: Passive uses last damage source only
            // (C++ getNextMoodTarget Passive branch); else nearest legal enemy.
            // Partition vision residual gates acquire distance.
            let mut best: Option<(ObjectId, f32, f32, f32)> = None; // id, dist, x, z
            if attitude.idle_mood_wait_for_attack() {
                if let Some(tid) = last_dmg {
                    if tid != cid {
                        if let Some(other) = self.objects.get(&tid) {
                            let op = other.get_position();
                            let dx = op.x - fire_pos.x;
                            let dz = op.z - fire_pos.z;
                            let dist = (dx * dx + dz * dz).sqrt();
                            let is_air =
                                other.is_kind_of(KindOf::Aircraft) || other.status.airborne_target;
                            if strategy_center_mood_target_enemy_legal_with_vision(
                                other.is_alive(),
                                other.team == team,
                                other.team == Team::Neutral,
                                other.status.under_construction,
                                is_air,
                                dist,
                                vision_range,
                            ) {
                                best = Some((tid, dist, op.x, op.z));
                            }
                        }
                    }
                }
            } else {
                // Pure residual acquire: nearest legal enemy in mood vision (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&oid, other)| {
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id: oid,
                            team: other.team,
                            position: other.get_position(),
                            is_alive: other.is_alive(),
                            is_neutral: other.team == Team::Neutral,
                            under_construction: other.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: other.is_effectively_stealthed(),
                            is_air: other.is_kind_of(KindOf::Aircraft)
                                || other.status.airborne_target,
                            eject_invulnerable: other.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(cid),
                    (fire_pos.x, fire_pos.z),
                    candidates,
                    vision_range,
                    |c| {
                        let dist = {
                            let dx = c.position.x - fire_pos.x;
                            let dz = c.position.z - fire_pos.z;
                            (dx * dx + dz * dz).sqrt()
                        };
                        strategy_center_mood_target_enemy_legal_with_vision(
                            c.is_alive,
                            c.team == team,
                            c.is_neutral,
                            c.under_construction,
                            c.is_air,
                            dist,
                            vision_range,
                        )
                    },
                )
                .map(|(id, dist, _)| {
                    let p = self
                        .objects
                        .get(&id)
                        .map(|o| o.get_position())
                        .unwrap_or(fire_pos);
                    (id, dist, p.x, p.z)
                });
            }
            if let Some((tid, _, tx, tz)) = best {
                let (aim_a, aim_p) = strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                if let Some(o) = self.objects.get_mut(&cid) {
                    o.set_target(Some(tid));
                    o.turret_mood_target = true;
                    o.turret_angle_deg = aim_a;
                    o.record_host_turret();
                    o.turret_pitch_deg = aim_p;
                    o.record_host_turret();
                    // Mood acquire cancels idle-scan residual.
                    o.turret_idle_scanning = false;
                    o.record_host_turret();
                    o.turret_holding = false;
                    o.record_host_turret();
                    o.turret_hold_until_frame = 0;
                    o.turret_idle_recentering = false;
                    o.set_ai_state(AIState::Attacking);
                    o.set_status_attacking(true);
                }
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(cid, tid);
                    crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                }
                acquires = acquires.saturating_add(1);
            }
        }
        for _ in 0..acquires {
            self.battle_plans.record_turret_mood_target_acquire();
        }
        for _ in 0..clears {
            self.battle_plans.record_turret_mood_target_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{AIState, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    /// C++ NeutonBlastBehavior.cpp:124-127 — unmanned vehicles are aiIdle'd
    /// and deselectObject'd so they do not stay selected or keep orders.
    #[test]
    fn neutron_unman_deselects_and_idles_ai() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));

        let mut tank = ThingTemplate::new("NeutronUnmanTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0);
        logic
            .templates
            .insert("NeutronUnmanTank".to_string(), tank);

        let tank_id = logic
            .create_object(
                "NeutronUnmanTank",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("tank");
        logic.select_objects(0, vec![tank_id]);
        {
            let obj = logic.host_object_mut(tank_id).expect("tank mut");
            obj.set_ai_state(AIState::Moving);
            obj.target_location = Some(Vec3::new(80.0, 0.0, 0.0));
            obj.set_status_moving(true);
            assert!(obj.selected || obj.status.selected);
            assert!(matches!(obj.ai_state, AIState::Moving));
        }
        assert!(
            logic
                .players
                .get(&0)
                .unwrap()
                .selected_objects
                .contains(&tank_id)
        );

        let (kills, unmanned, vehicle_kills) =
            logic.apply_neutron_blast_at(Vec3::ZERO, Team::China, None, true);
        assert_eq!(kills, 0);
        assert_eq!(unmanned, 1);
        assert_eq!(vehicle_kills, 0);

        let obj = logic.host_object(tank_id).expect("husk");
        assert!(obj.is_unmanned(), "neutron must unman the vehicle");
        assert_eq!(obj.team, Team::Neutral);
        assert!(!obj.selected, "C++ deselectObject must clear object.selected");
        assert!(
            !obj.status.selected,
            "C++ deselectObject must clear status.selected"
        );
        assert!(
            matches!(obj.ai_state, AIState::Idle),
            "C++ aiIdle(CMD_FROM_AI) must idle the husk"
        );
        assert!(
            obj.target_location.is_none(),
            "idle unman must drop pending move orders"
        );
        assert!(
            !logic
                .players
                .get(&0)
                .unwrap()
                .selected_objects
                .contains(&tank_id),
            "PLAYERMASK_ALL deselect must drop the husk from the player roster"
        );
        assert!(!logic.selected_objects.contains(&tank_id));
    }

    /// C++ OverlordContain.cpp:553 — BattleBunker infantry fire from the tank.
    #[test]
    fn overlord_bunker_infantry_residual_fire_without_helix_flag() {
        let mut logic = GameLogic::new();
        let mut overlord = ThingTemplate::new("ChinaTankOverlord");
        overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1100.0);
        logic
            .templates
            .insert("ChinaTankOverlord".to_string(), overlord);
        let mut red = ThingTemplate::new("ChinaRedguard");
        red.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(120.0);
        logic.templates.insert("ChinaRedguard".to_string(), red);
        let mut enemy = ThingTemplate::new("UsaRanger");
        enemy
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0);
        logic.templates.insert("UsaRanger".to_string(), enemy);

        let tank = logic
            .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("overlord");
        {
            let o = logic.host_object_mut(tank).unwrap();
            o.install_overlord_battle_bunker(5);
            o.passengers_allowed_to_fire = false;
        }
        let rider = logic
            .create_object("ChinaRedguard", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("rider");
        {
            let o = logic.host_object_mut(tank).unwrap();
            assert!(o.add_occupant(rider), "bunker must accept infantry");
        }
        {
            let r = logic.host_object_mut(rider).unwrap();
            r.contained_by = Some(tank);
            r.set_ai_state(AIState::Docked);
            if r.weapon.is_none() {
                r.weapon = Some(crate::game_logic::Weapon::default());
            }
            if let Some(w) = r.weapon.as_mut() {
                w.last_fire_time = -10.0;
                w.reload_time = 0.1;
                w.range = 150.0;
                w.damage = 10.0;
            }
        }
        let victim = logic
            .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("victim");
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_transport_passenger_residual_fire(rider);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        assert!(
            hp_after < hp_before - 0.01,
            "bunker infantry must fire (before={hp_before} after={hp_after})"
        );
        assert!(
            logic.host_object(tank).unwrap().passengers_allowed_to_fire,
            "live bunker fire sets passengers_allowed_to_fire"
        );
    }

    /// C++ TransportContain::isPassengerAllowedToFire — vehicles ride silent.
    #[test]
    fn combat_chinook_vehicle_rider_does_not_residual_fire() {
        let mut logic = GameLogic::new();
        let mut chinook = ThingTemplate::new("AirF_AmericaVehicleChinook");
        chinook
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0);
        logic
            .templates
            .insert("AirF_AmericaVehicleChinook".to_string(), chinook);
        let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
        humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".to_string(), humvee);
        let mut enemy = ThingTemplate::new("GLAVehicleTechnical");
        enemy
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0);
        logic
            .templates
            .insert("GLAVehicleTechnical".to_string(), enemy);

        let bird = logic
            .create_object(
                "AirF_AmericaVehicleChinook",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("chinook");
        {
            let c = logic.host_object_mut(bird).unwrap();
            c.install_combat_chinook_transport();
        }
        let rider = logic
            .create_object(
                "AmericaVehicleHumvee",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("humvee");
        {
            let o = logic.host_object_mut(bird).unwrap();
            assert!(o.add_occupant(rider), "Combat Chinook admits vehicles");
        }
        {
            let r = logic.host_object_mut(rider).unwrap();
            r.contained_by = Some(bird);
            r.set_ai_state(AIState::Docked);
            r.weapon = Some(crate::game_logic::Weapon {
                last_fire_time: -10.0,
                reload_time: 0.1,
                range: 150.0,
                damage: 40.0,
                ..crate::game_logic::Weapon::default()
            });
        }
        let victim = logic
            .create_object(
                "GLAVehicleTechnical",
                Team::GLA,
                Vec3::new(20.0, 0.0, 0.0),
            )
            .expect("victim");
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_transport_passenger_residual_fire(rider);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        assert!(
            (hp_after - hp_before).abs() < 0.01,
            "vehicle rider must not fire out of Combat Chinook (before={hp_before} after={hp_after})"
        );
    }


    fn garrison_template(name: &str, immune: bool, enclosing: bool) -> ThingTemplate {
        use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(5),
            admission: ContainAdmission::InfantryOnly,
            immune_to_clear_building_attacks: immune,
            is_enclosing_container: enclosing,
            ..ContainModuleMetadata::default()
        };
        t
    }

    #[test]
    fn immune_to_clear_bunker_keeps_occupants() {
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert(
                "ChinaBunker".into(),
                garrison_template("ChinaBunker", true, true),
            );
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);

        let bunker = logic
            .create_object("ChinaBunker", Team::China, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::China, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(bunker));
        }
        let killed = logic.apply_kill_garrisoned_to_target(bunker, Team::USA, 5.0, None);
        assert_eq!(killed, 0, "ImmuneToClear bunkers keep occupants");
        assert!(logic.host_object(ranger_id).unwrap().is_alive());
        assert_eq!(
            logic.host_object(bunker).unwrap().contained_units(),
            vec![ranger_id]
        );
    }

    /// C++ MicrowaveTankBuildingClearer DelayBetweenShots 100ms → 3f; 1 occupant/shot.
    #[test]
    fn microwave_clearer_delay_is_100ms_one_occupant_per_shot() {
        use crate::game_logic::host_microwave::{
            HOST_MICROWAVE_CLEAR_PER_SHOT, HOST_MICROWAVE_DELAY_FRAMES, MICROWAVE_LOGIC_FPS,
        };
        use crate::game_logic::weapon_bootstrap::{
            ensure_host_weapon_store, MICROWAVE_BUILDING_CLEARER_WEAPON,
        };

        ensure_host_weapon_store();
        let w = ThingTemplate::weapon_from_store(MICROWAVE_BUILDING_CLEARER_WEAPON)
            .expect("MicrowaveTankBuildingClearer seeded");
        let expected = HOST_MICROWAVE_DELAY_FRAMES as f32 / MICROWAVE_LOGIC_FPS;
        assert!(
            (w.reload_time - expected).abs() < 1e-3,
            "clearer DelayBetweenShots 100ms → reload {}, got {}",
            expected,
            w.reload_time
        );
        assert!((w.damage - HOST_MICROWAVE_CLEAR_PER_SHOT).abs() < 1e-3);

        let mut logic = GameLogic::new();
        logic.templates.insert(
            "ChinaBunker".into(),
            garrison_template("ChinaBunker", false, true),
        );
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);

        let bunker = logic
            .create_object("ChinaBunker", Team::China, Vec3::ZERO)
            .unwrap();
        let a = logic
            .create_object("AmericaRanger", Team::China, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("AmericaRanger", Team::China, Vec3::new(6.0, 0.0, 0.0))
            .unwrap();
        {
            let o = logic.host_object_mut(bunker).unwrap();
            assert!(o.add_occupant(a));
            assert!(o.add_occupant(b));
        }
        for id in [a, b] {
            if let Some(r) = logic.host_object_mut(id) {
                r.set_contained_by(Some(bunker));
            }
        }
        let killed = logic.apply_kill_garrisoned_to_target(
            bunker,
            Team::USA,
            HOST_MICROWAVE_CLEAR_PER_SHOT,
            None,
        );
        assert_eq!(killed, 1, "PrimaryDamage 1 kills one occupant per 100ms shot");
        assert_eq!(
            logic.host_object(bunker).unwrap().contained_units().len(),
            1
        );
        logic.microwaves.record_clear_shot();
        assert!(logic.microwave_residual().clear_shots > 0);
    }


    #[test]
    fn occupied_building_gets_can_attack_and_loses_it_when_empty() {
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert(
                "CivBunker".into(),
                garrison_template("CivBunker", false, true),
            );
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger.add_kind_of(KindOf::Infantry).set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        logic.apply_garrison_contain_on_enter(bunker, ranger_id);
        {
            let b = logic.host_object(bunker).unwrap();
            assert!(b.has_object_status_bit("CAN_ATTACK"));
            assert!(b.can_attack(), "occupied garrison must accept attack orders");
        }
        assert!(logic.host_object_mut(bunker).unwrap().remove_occupant(ranger_id));
        let b = logic.host_object(bunker).unwrap();
        assert!(!b.has_object_status_bit("CAN_ATTACK"));
    }

    #[test]
    fn garrison_fire_point_is_not_eight_point_ring() {
        use crate::game_logic::ContainModuleKind;
        let mut t = garrison_template("CivBunker", false, true);
        t.model_name = Some("nosuchmodel".into());
        let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
        obj.set_position(Vec3::new(10.0, 0.0, 20.0));
        obj.building_data = Some(crate::game_logic::BuildingData::new(
            crate::game_logic::BuildingType::Bunker,
        ));
        assert_eq!(
            obj.thing.template.contain_module.kind,
            ContainModuleKind::Garrison
        );
        let (idx, pos) =
            garrison_occupant_fire_point(&obj, crate::game_logic::ObjectId(2), Vec3::new(100.0, 0.0, 20.0));
        assert_eq!(idx, 0);
        // C++ with no FIREPOINT bones uses the building origin, not a r=12 ring.
        assert!((pos - obj.get_position()).length() < 0.01);
    }

    #[test]
    fn script_evac_left_is_not_a_circle() {
        let origin = Vec3::ZERO;
        let (start, end) = super::super::registries::garrison_evac_side_points_for_test(
            origin,
            0.0,
            20.0,
            10.0,
            1,
            1,
        );
        assert!(
            end.z.abs() >= 50.0,
            "left evac must spread along the side, not an 8-unit ring"
        );
        assert!(start.z.signum() == end.z.signum() || start.z.abs() > 0.0);
    }

    #[test]
    fn fire_base_is_not_enclosing() {
        let t = garrison_template("AmericaFireBase", false, false);
        assert!(!t.contain_module.is_enclosing_container);
        let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(3), Team::USA);
        obj.building_data = Some(crate::game_logic::BuildingData::new(
            crate::game_logic::BuildingType::Bunker,
        ));
        assert!(!obj.is_enclosing_garrison_container());
    }

    #[test]
    fn garrison_enter_deselects_occupant() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.select();
            r.owner_player_id = Some(0);
        }
        logic.players.get_mut(&0).unwrap().selected_objects.push(ranger_id);
        logic.selected_objects.push(ranger_id);
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(bunker));
            r.deselect();
        }
        logic.players.get_mut(&0).unwrap().selected_objects.retain(|id| *id != ranger_id);
        logic.selected_objects.retain(|id| *id != ranger_id);
        let r = logic.host_object(ranger_id).unwrap();
        assert!(!r.selected);
        assert!(!logic.selected_objects.contains(&ranger_id));
    }

    fn infantry_template(name: &str) -> ThingTemplate {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(120.0);
        t.transport_slot_count = Some(1);
        t
    }

    #[test]
    fn hide_uses_stealth_garrison_kind_not_stealthed_bits() {
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        let mut ranger = infantry_template("AmericaRanger");
        ranger.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AmericaRanger".into(), ranger);
        let mut ninja = infantry_template("JapanNinja");
        ninja.add_kind_of(KindOf::StealthGarrison);
        logic.templates.insert("JapanNinja".into(), ninja);

        let bunker = logic
            .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.status.stealthed = true;
            r.status.detected = false;
        }
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(bunker));
        }
        logic.recalc_garrison_apparent_controller(bunker);
        assert!(
            !logic
                .host_object(bunker)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .hide_garrisoned_state,
            "ordinary stealthed infantry must not hide the building"
        );

        assert!(logic.host_object_mut(bunker).unwrap().remove_occupant(ranger_id));
        let ninja_id = logic
            .create_object("JapanNinja", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        if let Some(n) = logic.host_object_mut(ninja_id) {
            n.status.stealthed = false;
            n.status.detected = false;
        }
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ninja_id));
        if let Some(n) = logic.host_object_mut(ninja_id) {
            n.set_contained_by(Some(bunker));
        }
        logic.recalc_garrison_apparent_controller(bunker);
        assert!(
            logic
                .host_object(bunker)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .hide_garrisoned_state,
            "STEALTH_GARRISON kind hides even while destalthed and not DETECTED"
        );
    }

    #[test]
    fn enemy_may_enter_stealth_garrison_only_civilian_and_kick() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
        logic
            .players
            .insert(1, Player::new(1, Team::China, "China", false));
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        let mut ninja = infantry_template("JapanNinja");
        ninja.add_kind_of(KindOf::StealthGarrison);
        logic.templates.insert("JapanNinja".into(), ninja);
        logic
            .templates
            .insert("ChinaRedguard".into(), infantry_template("ChinaRedguard"));

        let bunker = logic
            .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
            .unwrap();
        if let Some(b) = logic.host_object_mut(bunker) {
            b.owner_player_id = None;
        }
        let ninja_id = logic
            .create_object("JapanNinja", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        if let Some(n) = logic.host_object_mut(ninja_id) {
            n.owner_player_id = Some(0);
            n.status.detected = false;
        }
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ninja_id));
        if let Some(n) = logic.host_object_mut(ninja_id) {
            n.set_contained_by(Some(bunker));
        }
        let enemy = logic
            .create_object("ChinaRedguard", Team::China, Vec3::new(3.0, 0.0, 0.0))
            .unwrap();
        if let Some(e) = logic.host_object_mut(enemy) {
            e.owner_player_id = Some(1);
        }
        assert!(
            logic.can_unit_enter_normal_target(enemy, bunker),
            "C++ lets a non-owner Enter a stealth-garrison-only civilian"
        );
        logic.kick_other_controller_occupants_for_enter(bunker, enemy);
        let ninja = logic.host_object(ninja_id).unwrap();
        assert!(ninja.status.detected, "STEALTH_GARRISON kick markAsDetected");
        assert!(ninja.contained_by.is_none());
        assert!(
            matches!(ninja.ai_state, AIState::Moving),
            "kicked occupant must walk out, not idle"
        );
        assert!(logic.host_object(bunker).unwrap().contained_units().is_empty());
    }

    #[test]
    fn evac_burst_walks_out_instead_of_idling_at_origin() {
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::new(10.0, 0.0, 20.0))
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(12.0, 0.0, 20.0))
            .unwrap();
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(bunker));
        }
        assert!(logic.evacuate_container_now(bunker, false));
        let r = logic.host_object(ranger_id).unwrap();
        assert!(matches!(r.ai_state, AIState::Moving));
        assert!(r.status.moving);
        let dest = r.movement.target_position.unwrap_or(r.get_position());
        assert!(
            (dest - Vec3::new(10.0, 0.0, 20.0)).length() > 1.0,
            "burst dest must leave the building origin"
        );
    }

    #[test]
    fn really_damaged_garrison_rejects_enter_unless_firebase() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        let mut firebase = garrison_template("AmericaFireBase", false, false);
        firebase.add_kind_of(KindOf::GarrisonableUntilDestroyed);
        logic.templates.insert("AmericaFireBase".into(), firebase);
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let ranger = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.can_unit_enter_normal_target(ranger, bunker));
        {
            let b = logic.host_object_mut(bunker).unwrap();
            b.health.current = 200.0;
            b.refresh_model_condition_bits();
            assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
        }
        assert!(
            !logic.can_unit_enter_normal_target(ranger, bunker),
            "C++ isValidContainerFor rejects BODY_REALLYDAMAGED civilian/faction buildings"
        );

        let fb = logic
            .create_object("AmericaFireBase", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        {
            let b = logic.host_object_mut(fb).unwrap();
            b.health.current = 200.0;
            b.refresh_model_condition_bits();
            assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
        }
        assert!(
            logic.can_unit_enter_normal_target(ranger, fb),
            "KINDOF_GARRISONABLE_UNTIL_DESTROYED stays occupiable through ReallyDamaged"
        );
    }

    #[test]
    fn really_damaged_ejects_garrison_with_burst_walk() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("CivBunker".into(), garrison_template("CivBunker", false, true));
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::new(10.0, 0.0, 20.0))
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(12.0, 0.0, 20.0))
            .unwrap();
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(bunker));
            r.set_ai_state(AIState::Garrisoned);
        }
        {
            let b = logic.host_object_mut(bunker).unwrap();
            b.health.current = 200.0;
            b.refresh_model_condition_bits();
            assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
        }
        logic.check_building_damage_states(&[bunker]);
        let r = logic.host_object(ranger_id).unwrap();
        assert!(r.contained_by.is_none());
        assert!(
            matches!(r.ai_state, AIState::Moving),
            "ReallyDamaged eject must walk out, not Idle on an 8-unit ring"
        );
        assert!(r.status.moving);
        let dest = r.movement.target_position.unwrap_or(r.get_position());
        assert!(
            (dest - Vec3::new(10.0, 0.0, 20.0)).length() > 1.0,
            "burst dest must leave the building origin"
        );
        assert!(logic.host_object(bunker).unwrap().contained_units().is_empty());
    }

    #[test]
    fn firebase_really_damaged_does_not_eject() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut logic = GameLogic::new();
        let mut firebase = garrison_template("AmericaFireBase", false, false);
        firebase.add_kind_of(KindOf::GarrisonableUntilDestroyed);
        logic.templates.insert("AmericaFireBase".into(), firebase);
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        let fb = logic
            .create_object("AmericaFireBase", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.host_object_mut(fb).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(fb));
            r.set_ai_state(AIState::Garrisoned);
        }
        {
            let b = logic.host_object_mut(fb).unwrap();
            b.health.current = 200.0;
            b.refresh_model_condition_bits();
            assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
        }
        logic.check_building_damage_states(&[fb]);
        assert_eq!(
            logic.host_object(ranger_id).unwrap().contained_by,
            Some(fb),
            "GARRISONABLE_UNTIL_DESTROYED must keep occupants through ReallyDamaged"
        );
    }


    #[test]
    fn garrison_fire_points_switch_with_body_damage() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let t = garrison_template("CivBunker", false, true);
        let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
        obj.set_position(Vec3::new(10.0, 0.0, 20.0));
        let mut bd = crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
        bd.garrison_fire_points = vec![Vec3::new(11.0, 0.0, 20.0)];
        bd.garrison_fire_points_damaged = vec![Vec3::new(30.0, 0.0, 20.0)];
        bd.garrison_fire_points_really_damaged = vec![Vec3::new(50.0, 0.0, 20.0)];
        bd.garrison_point_occupant = vec![None];
        obj.building_data = Some(bd);
        obj.body_damage_state = HostBodyDamageType::Pristine;
        let (_, p0) = garrison_occupant_fire_point(&obj, crate::game_logic::ObjectId(2), Vec3::new(100.0, 0.0, 20.0));
        assert!((p0 - Vec3::new(11.0, 0.0, 20.0)).length() < 0.01);
        obj.body_damage_state = HostBodyDamageType::Damaged;
        let (_, p1) = garrison_occupant_fire_point(&obj, crate::game_logic::ObjectId(2), Vec3::new(100.0, 0.0, 20.0));
        assert!((p1 - Vec3::new(30.0, 0.0, 20.0)).length() < 0.01);
        obj.body_damage_state = HostBodyDamageType::ReallyDamaged;
        let (_, p2) = garrison_occupant_fire_point(&obj, crate::game_logic::ObjectId(2), Vec3::new(100.0, 0.0, 20.0));
        assert!((p2 - Vec3::new(50.0, 0.0, 20.0)).length() < 0.01);
    }

    #[test]
    fn transport_fire_origin_uses_firepoint_or_hull() {
        let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
        humvee
            .add_kind_of(KindOf::Vehicle)
            .set_health(240.0);
        humvee.model_name = Some("nosuchmodel".into());
        let mut obj = crate::game_logic::Object::new(humvee, crate::game_logic::ObjectId(9), Team::USA);
        obj.set_position(Vec3::new(7.0, 0.0, 3.0));
        let origin = transport_passenger_fire_origin(&obj, 0);
        assert!(
            (origin - obj.get_position()).length() < 0.01,
            "no FIREPOINT bones → hull center (C++ m_noFirePointsInArt)"
        );
    }

    #[test]
    fn helix_fire_origin_is_hull_plus_eight_not_firepoint() {
        let mut helix = ThingTemplate::new("ChinaVehicleHelix");
        helix
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .set_health(600.0);
        helix.model_name = Some("nosuchmodel".into());
        let mut obj =
            crate::game_logic::Object::new(helix, crate::game_logic::ObjectId(8), Team::China);
        obj.install_helix_transport();
        obj.set_position(Vec3::new(7.0, 10.0, 3.0));
        let origin = transport_passenger_fire_origin(&obj, 0);
        let expected = obj.get_position() + Vec3::new(0.0, HELIX_OCCUPANT_FIRE_HEIGHT, 0.0);
        assert!(
            (origin - expected).length() < 0.01,
            "HelixContain::redeployOccupants is hull+8 (host Y), not FIREPOINT: {origin:?}"
        );
    }


    #[test]
    fn heal_contain_auto_exit_walks_exit_path() {
        use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
        let mut pad = ThingTemplate::new("AmericaBarracks");
        pad.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::HealPad)
            .set_health(800.0);
        pad.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Heal,
            slots: Some(10),
            admission: ContainAdmission::InfantryOnly,
            frames_for_full_heal: Some(0),
            ..ContainModuleMetadata::default()
        };
        logic.templates.insert("AmericaBarracks".into(), pad);
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        let barracks = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.health.current = 20.0;
            r.owner_player_id = Some(0);
        }
        assert!(logic.host_object_mut(barracks).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.set_contained_by(Some(barracks));
        }
        logic.tunnel_network.stamp_contained_by_frame(ranger_id, 0);
        logic.frame = 1;
        logic.update_support_states(&[ranger_id], 1.0 / 30.0);
        let r = logic.host_object(ranger_id).unwrap();
        assert!(r.contained_by.is_none());
        assert!(
            matches!(r.ai_state, AIState::Moving),
            "HealContain auto-exit must follow ExitStart/End, not Idle on an 8-unit circle"
        );
        assert!(r.status.moving);
    }

    #[test]
    fn open_contain_exit_path_cycles_numbered_like_cpp() {
        let mut t = ThingTemplate::new("HV_EXIT");
        t.add_kind_of(KindOf::Vehicle);
        t.set_health(200.0);
        let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
        obj.set_position(Vec3::new(10.0, 0.0, 4.0));
        obj.set_orientation(0.0);
        let origin = obj.get_position();
        let (s1, e1, n1) = open_contain_exit_path(&obj, 0, 3);
        assert_eq!(n1, 2, "C++ m_whichExitPath cycles 1→2 after ExitStart01");
        assert!((s1 - origin).length() < 0.01, "missing bone → hull start");
        assert!(
            (e1 - origin).length() > 8.0,
            "missing bone → forward ExitEnd, not Idle ring: e1={e1:?}"
        );
        let (_, _, n2) = open_contain_exit_path(&obj, n1, 3);
        assert_eq!(n2, 3);
        let (_, _, n3) = open_contain_exit_path(&obj, n2, 3);
        assert_eq!(n3, 1);
        let (_, e_single, next_single) = open_contain_exit_path(&obj, 1, 1);
        assert_eq!(next_single, 1);
        assert!((e_single - origin).length() > 8.0);
    }

    #[test]
    fn walk_unit_via_open_contain_exit_cycles_humvee_paths() {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("HV_CYCLE");
        t.add_kind_of(KindOf::Vehicle);
        t.set_health(200.0);
        t.contain_module = crate::game_logic::ContainModuleMetadata {
            kind: crate::game_logic::ContainModuleKind::Transport,
            slots: Some(5),
            ..Default::default()
        };
        logic.templates.insert("HV_CYCLE".into(), t);
        let mut p = ThingTemplate::new("HV_CYCLE_P");
        p.add_kind_of(KindOf::Infantry);
        p.set_health(100.0);
        logic.templates.insert("HV_CYCLE_P".into(), p);
        let transport = logic
            .create_object("HV_CYCLE", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        {
            logic
                .host_object_mut(transport)
                .unwrap()
                .install_humvee_transport();
        }
        let a = logic
            .create_object("HV_CYCLE_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("HV_CYCLE_P", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        logic.walk_unit_via_open_contain_exit(a, transport);
        assert_eq!(
            logic.host_object(transport).unwrap().which_exit_path,
            2,
            "first rider consumes ExitStart01"
        );
        logic.walk_unit_via_open_contain_exit(b, transport);
        assert_eq!(
            logic.host_object(transport).unwrap().which_exit_path,
            3,
            "second rider consumes ExitStart02"
        );
        assert_eq!(logic.host_object(a).unwrap().ai_state, AIState::Moving);
        assert_eq!(logic.host_object(b).unwrap().ai_state, AIState::Moving);
    }


    /// C++ ActionManager.cpp:1696-1710 + Object.cpp:6111-6132.
    #[test]
    fn defector_rejects_structure_contained_and_unfinished() {
        let mut logic = GameLogic::new();
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure).set_health(1000.0);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let mut tank = ThingTemplate::new("TestTank");
        tank.add_kind_of(KindOf::Vehicle).set_health(200.0);
        logic.templates.insert("TestTank".into(), tank);
        let mut barracks = ThingTemplate::new("GLABarracks");
        barracks.add_kind_of(KindOf::Structure).set_health(800.0);
        logic.templates.insert("GLABarracks".into(), barracks);

        let caster = logic
            .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
            .unwrap();
        let building = logic
            .create_object("GLABarracks", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        let unfinished = logic
            .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.host_object_mut(unfinished) {
            o.set_status_under_construction(true);
            o.construction_percent = 0.2;
        }
        let contained = logic
            .create_object("TestTank", Team::GLA, Vec3::new(60.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.host_object_mut(contained) {
            o.set_contained_by(Some(building));
        }
        let sold = logic
            .create_object("TestTank", Team::GLA, Vec3::new(70.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.host_object_mut(sold) {
            o.set_status_sold(true);
        }

        assert!(!logic.activate_defector(caster, building));
        assert_eq!(logic.host_object(building).unwrap().team, Team::GLA);
        assert!(!logic.activate_defector(caster, unfinished));
        assert_eq!(logic.host_object(unfinished).unwrap().team, Team::GLA);
        assert!(!logic.activate_defector(caster, contained));
        assert_eq!(logic.host_object(contained).unwrap().team, Team::GLA);
        assert!(!logic.activate_defector(caster, sold));
        assert_eq!(logic.host_object(sold).unwrap().team, Team::GLA);
    }

    /// C++ Object.cpp:6167-6192 — idle + VoiceDefect + kickOutOnCapture.
    #[test]
    fn defector_idles_and_kicks_cargo() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
        logic
            .players
            .insert(2, Player::new(2, Team::GLA, "GLA", false));
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure).set_health(1000.0);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
        humvee.add_kind_of(KindOf::Vehicle).set_health(200.0);
        logic.templates.insert("AmericaVehicleHumvee".into(), humvee);
        let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let caster = logic
            .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
            .unwrap();
        let victim = logic
            .create_object(
                "AmericaVehicleHumvee",
                Team::GLA,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .unwrap();
        let cargo = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::GLA,
                Vec3::new(42.0, 0.0, 0.0),
            )
            .unwrap();
        if let Some(v) = logic.host_object_mut(victim) {
            v.set_ai_state(AIState::Attacking);
            v.set_target(Some(caster));
            v.set_status_attacking(true);
            v.occupants.push(cargo);
        }
        if let Some(c) = logic.host_object_mut(cargo) {
            c.set_contained_by(Some(victim));
        }

        assert!(logic.activate_defector(caster, victim));
        let v = logic.host_object(victim).unwrap();
        assert_eq!(v.team, Team::USA);
        assert!(matches!(v.ai_state, AIState::Idle));
        assert!(!v.status.attacking);
        assert!(v.target.is_none());
        assert!(v.is_undetected_defector());
        let rider = logic.host_object(cargo).unwrap();
        assert!(rider.contained_by.is_none());
        assert_eq!(rider.team, Team::GLA);
    }

    /// hq-am2jn: garrison auto-fire must skip undetected stealth (C++ acquire filters).
    #[test]
    fn garrison_residual_fire_skips_undetected_stealth() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "CivBunker".into(),
            garrison_template("CivBunker", false, true),
        );
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        logic
            .templates
            .insert("GLARebel".into(), infantry_template("GLARebel"));

        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        let rebel = logic
            .create_object("GLARebel", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        {
            let r = logic.host_object_mut(ranger).unwrap();
            r.weapon = Some(crate::game_logic::Weapon {
                damage: 40.0,
                range: 100.0,
                reload_time: 0.1,
                last_fire_time: -10.0,
                ..crate::game_logic::Weapon::default()
            });
            r.set_contained_by(Some(bunker));
            r.set_ai_state(AIState::Garrisoned);
        }
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));
        {
            let e = logic.host_object_mut(rebel).unwrap();
            e.set_status_stealthed(true);
            e.set_status_detected(false);
            assert!(e.is_effectively_stealthed());
        }

        let hp_before = logic.host_object(rebel).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_garrison_residual_fire(ranger);
        let hp_after = logic.host_object(rebel).unwrap().health.current;
        assert!(
            (hp_after - hp_before).abs() < 0.01,
            "undetected stealth must not be auto-acquired (before={hp_before} after={hp_after})"
        );
        assert_eq!(logic.garrison_residual_fires(), 0);

        {
            let e = logic.host_object_mut(rebel).unwrap();
            e.set_status_detected(true);
            assert!(!e.is_effectively_stealthed());
        }
        logic.try_garrison_residual_fire(ranger);
        let hp_detected = logic.host_object(rebel).unwrap().health.current;
        assert!(
            hp_detected < hp_before - 0.01,
            "detected stealth remains a legal acquire"
        );
    }

    /// hq-nzyae: garrison fire uses GARRISONED 133% range, not raw weapon.range.
    #[test]
    fn garrison_residual_fire_uses_garrisoned_133_range() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "CivBunker".into(),
            garrison_template("CivBunker", false, true),
        );
        logic
            .templates
            .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
        logic
            .templates
            .insert("GLATank".into(), infantry_template("GLATank"));

        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        // 120 units: out of raw 100, inside 100 * 1.33.
        let enemy = logic
            .create_object("GLATank", Team::GLA, Vec3::new(120.0, 0.0, 0.0))
            .unwrap();
        {
            let r = logic.host_object_mut(ranger).unwrap();
            r.weapon = Some(crate::game_logic::Weapon {
                damage: 25.0,
                range: 100.0,
                reload_time: 0.1,
                last_fire_time: -10.0,
                ..crate::game_logic::Weapon::default()
            });
            r.set_contained_by(Some(bunker));
            r.set_ai_state(AIState::Garrisoned);
            assert!(
                (r.effective_weapon_range(100.0) - 133.0).abs() < 0.01,
                "Garrisoned infantry must receive RANGE 133%"
            );
        }
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));

        let hp_before = logic.host_object(enemy).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_garrison_residual_fire(ranger);
        let hp_after = logic.host_object(enemy).unwrap().health.current;
        assert!(
            hp_after < hp_before - 0.01,
            "garrisoned 133% range must reach 120 with base 100 (before={hp_before} after={hp_after})"
        );
        assert!(logic.honesty_garrison_fire_ok());
    }

    /// hq-nzyae: Helix infantry stay Docked but still get GARRISONED 133% range.
    #[test]
    fn helix_infantry_residual_fire_uses_garrisoned_133_range() {
        let mut logic = GameLogic::new();
        let mut helix = ThingTemplate::new("ChinaHelix");
        helix
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(600.0);
        logic.templates.insert("ChinaHelix".into(), helix);
        logic
            .templates
            .insert("ChinaRedguard".into(), infantry_template("ChinaRedguard"));
        logic
            .templates
            .insert("UsaRanger".into(), infantry_template("UsaRanger"));

        let heli = logic
            .create_object("ChinaHelix", Team::China, Vec3::ZERO)
            .unwrap();
        {
            let h = logic.host_object_mut(heli).unwrap();
            h.install_helix_transport();
            h.passengers_allowed_to_fire = true;
        }
        let rider = logic
            .create_object("ChinaRedguard", Team::China, Vec3::ZERO)
            .unwrap();
        assert!(logic.host_object_mut(heli).unwrap().add_occupant(rider));
        {
            let r = logic.host_object_mut(rider).unwrap();
            r.set_contained_by(Some(heli));
            r.set_ai_state(AIState::Docked);
            r.weapon = Some(crate::game_logic::Weapon {
                damage: 20.0,
                range: 100.0,
                reload_time: 0.1,
                last_fire_time: -10.0,
                ..crate::game_logic::Weapon::default()
            });
            // Docked + contained must not grant the bunker AIState bonus.
            assert!((r.effective_weapon_range(100.0) - 100.0).abs() < 0.01);
        }
        let victim = logic
            .create_object("UsaRanger", Team::USA, Vec3::new(120.0, 0.0, 0.0))
            .unwrap();
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_transport_passenger_residual_fire(rider);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        assert!(
            hp_after < hp_before - 0.01,
            "Helix infantry GARRISONED 133% must reach 120 with base 100 (before={hp_before} after={hp_after})"
        );
    }

    #[test]
    fn garrison_initial_roster_spawns_occupants_on_create() {
        let mut logic = GameLogic::new();
        let mut bunker = garrison_template("RosterBunker", false, true);
        bunker.contain_module.initial_roster_template = "AmericaRanger".to_string();
        bunker.contain_module.initial_roster_count = 3;
        logic.templates.insert("RosterBunker".into(), bunker);
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);

        let bunker_id = logic
            .create_object("RosterBunker", Team::USA, Vec3::ZERO)
            .expect("roster bunker");
        let occupants = logic
            .host_object(bunker_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        assert_eq!(
            occupants.len(),
            3,
            "C++ GarrisonContain::onObjectCreated must add InitialRoster count"
        );
        for occupant_id in occupants {
            let occupant = logic.host_object(occupant_id).expect("roster occupant");
            assert_eq!(occupant.template_name, "AmericaRanger");
            assert_eq!(occupant.contained_by, Some(bunker_id));
            assert_eq!(occupant.team, Team::USA);
        }
    }

    #[test]
    fn garrison_without_heal_objects_does_not_heal_occupants() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "CivBunker".into(),
            garrison_template("CivBunker", false, true),
        );
        let mut ranger = ThingTemplate::new("AmericaRanger");
        ranger.add_kind_of(KindOf::Infantry).set_health(120.0);
        logic.templates.insert("AmericaRanger".into(), ranger);
        let bunker = logic
            .create_object("CivBunker", Team::USA, Vec3::ZERO)
            .unwrap();
        let ranger_id = logic
            .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger_id));
        if let Some(r) = logic.host_object_mut(ranger_id) {
            r.health.current = 40.0;
            r.health.maximum = 120.0;
            r.set_contained_by(Some(bunker));
        }
        logic
            .tunnel_network
            .stamp_contained_by_frame(ranger_id, logic.frame);
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[ranger_id], 1.0 / 30.0);
        let after = logic.host_object(ranger_id).unwrap().health.current;
        assert!(
            (after - 40.0).abs() < 0.01,
            "HealObjects=No must not regenerate occupants, got {after}"
        );
    }
}

