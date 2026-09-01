//! Host combat `impl GameLogic` — `ocl_and_scud`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// Whether this host bridge can reproduce a parsed generic OCL nugget.
/// Unsupported physics/container/debris nuggets are skipped individually —
/// C++ `ObjectCreationList::createInternal` still runs every later nugget.
fn supports_host_weapon_generic_ocl(
    nugget: &gamelogic::object_creation_list::GenericObjectCreationNugget,
) -> bool {
    use gamelogic::object_creation_list::DebrisDisposition;

    let unsupported_disposition = DebrisDisposition::SEND_IT_FLYING
        | DebrisDisposition::SEND_IT_UP
        | DebrisDisposition::SEND_IT_OUT
        | DebrisDisposition::RANDOM_FORCE
        | DebrisDisposition::FLOATING
        | DebrisDisposition::WHIRLING;

    nugget.name_are_objects
        && !nugget.names.is_empty()
        && nugget.debris_to_generate > 0
        && !nugget.disposition.has(unsupported_disposition)
        && nugget.put_in_container.trim().is_empty()
        && !nugget.ignore_primary_obstacle
        && !nugget.contain_inside_source_object
        && !nugget.spread_formation
        && !nugget.dies_on_bad_land
        && !nugget.requires_live_player
}

fn usable_ocl_particle_sys_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then_some(name)
}

/// C++ `getPrimaryDamageRadius` residual for FireFX / DetonationFX overrideRadius.
fn host_fire_fx_override_radius(shooter: Option<&Object>) -> f32 {
    shooter
        .and_then(|o| o.weapon_name_for_slot(0))
        .map(crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name)
        .unwrap_or(0.0)
}

impl GameLogic {
    /// Execute a parsed `Weapon.ini` OCL at a source origin.
    ///
    /// C++ `Weapon::fireWeaponTemplate` calls `ObjectCreationList::create`
    /// with the firing object for `FireOCL`, and with the live missile object
    /// for `ProjectileDetonationOCL` (`Weapon.cpp:943-949`,
    /// `MissileAIUpdate.cpp:364-381`).  The host has no simulation `Object`
    /// for every visual projectile, so the caller supplies that object's
    /// frozen team/rank and its fire/impact origin.  References are resolved
    /// only through the parsed global OCL store; unresolved lists and object
    /// templates intentionally produce no guessed replacement.
    pub(crate) fn execute_parsed_weapon_ocl_at(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
    ) -> Vec<ObjectId> {
        use gamelogic::object_creation_list::{
            DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget,
        };

        let ocl_name = ocl_name.trim();
        if source_id.is_none() || ocl_name.is_empty() || ocl_name.eq_ignore_ascii_case("None") {
            return Vec::new();
        }

        let Some(ocl) =
            gamelogic::helpers::TheObjectCreationListStore::lookup_object_creation_list(ocl_name)
        else {
            return Vec::new();
        };

        // C++ ObjectCreationList.cpp:1524-1534 createInternal walks every nugget
        // independently. A leftover-physics CreateDebris/FireWeapon/etc. must not
        // discard earlier CreateObject results from the same authored list.
        let mut created = Vec::new();
        for nugget in ocl.nuggets() {
            let Some(generic) = nugget
                .as_any()
                .downcast_ref::<GenericObjectCreationNugget>()
            else {
                continue;
            };
            if !supports_host_weapon_generic_ocl(generic) {
                continue;
            }

            // `GenericObjectCreationNugget::create` checks source altitude
            // before spawning.  The host coordinate system is Y-up, whereas
            // C++ Coord3D is Z-up; terrain_height_at already accepts host X/Z.
            if generic.skip_if_significantly_airborne {
                let terrain_y = self.terrain_height_at(origin).unwrap_or(origin.y);
                if crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                    origin.y - terrain_y,
                ) {
                    continue;
                }
            }

            let count = usize::try_from(generic.debris_to_generate).unwrap_or_default();
            let upper_pick = match i32::try_from(generic.names.len().saturating_sub(1)) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for _ in 0..count {
                // C++ chooses once per created object from ObjectNames using
                // the game-logic random stream (ObjectCreationList.cpp:1326).
                let raw_pick = gamelogic::helpers::get_game_logic_random_value(0, upper_pick);
                let Ok(pick) = usize::try_from(raw_pick) else {
                    continue;
                };
                let Some(template_name) = generic.names.get(pick) else {
                    continue;
                };
                let template_name = template_name.trim();
                if template_name.is_empty() || template_name.eq_ignore_ascii_case("None") {
                    continue;
                }

                // `GameLogic::create_object` has a map/decorative fallback
                // path.  Weapon OCLs must not enter that path: first inject
                // only a real parsed asset definition, then create through the
                // normal world object lifecycle.
                if !self.templates.contains_key(template_name) {
                    let Some(template) = Self::build_template_from_asset_definition(template_name)
                    else {
                        continue;
                    };
                    self.templates.insert(template_name.to_string(), template);
                }

                // C++ `GenericObjectCreationNugget::doStuffToObj` transforms
                // `Offset` through the primary object's matrix before adding
                // it to the source position (ObjectCreationList.cpp:953-960).
                // `Coord3D` is (X, Y-horizontal, Z-up), whereas the host is
                // X/Z-horizontal with Y-up, so convert first and then apply
                // the frozen source yaw.  Leaving this unrotated puts every
                // directional FireOCL/debris spawn on the same world side of
                // its source regardless of facing.
                let local_offset = Vec3::new(generic.offset.x, generic.offset.z, generic.offset.y);
                let rotated_offset = glam::Mat3::from_rotation_y(source_orientation) * local_offset;
                let mut spawn_position = origin + rotated_offset;
                let on_ground = generic
                    .disposition
                    .has(DebrisDisposition::ON_GROUND_ALIGNED);
                if on_ground {
                    spawn_position.y = self
                        .terrain_height_at(spawn_position)
                        .unwrap_or(spawn_position.y);
                }

                // C++ ObjectCreationList.cpp:1302-1305
                // `sourceObj->getControllingPlayer()->getDefaultTeam()`.
                // `create_object` uses `unique_player_id_for_team`, which is
                // None in 2v2 same-faction, dropping the controlling player.
                let source_owner_player_id =
                    source_id.and_then(|id| self.objects.get(&id).and_then(|o| o.owner_player_id));
                let Some(created_id) = self.create_object_for_owner_or_team(
                    template_name,
                    source_team,
                    source_owner_player_id,
                    spawn_position,
                ) else {
                    continue;
                };
                let inherit_name = {
                    let Some(object) = self.objects.get_mut(&created_id) else {
                        continue;
                    };

                    object.producer_id = source_id;
                    if generic.disposition.has(DebrisDisposition::LIKE_EXISTING) {
                        object.set_orientation(source_orientation);
                    }
                    if generic.disposition.has(DebrisDisposition::INHERIT_VELOCITY) {
                        // The host's movement/physics state is the concrete
                        // equivalent of C++ PhysicsBehavior::applyForce for this
                        // disposition.  It also preserves DragonTank FireWall
                        // segment direction without inspecting an OCL name.
                        object.movement.velocity = source_velocity;
                    }
                    // C++ ObjectCreationList.cpp:996-1006 — inherit rank and
                    // transferObjectName only when the created tracker isTrainable.
                    let inherit_name = generic.inherit_veterancy && object.is_trainable();
                    if inherit_name {
                        object.set_min_veterancy_level(source_veterancy);
                    }
                    inherit_name
                };
                let Some(object) = self.objects.get_mut(&created_id) else {
                    continue;
                };
                if generic.max_frames > 0 {
                    let min_frames = i32::try_from(generic.min_frames).unwrap_or(i32::MAX);
                    let max_frames = i32::try_from(generic.max_frames).unwrap_or(i32::MAX);
                    let chosen_frames =
                        gamelogic::helpers::get_game_logic_random_value(min_frames, max_frames);
                    let delay_frames = u32::try_from(chosen_frames).unwrap_or(generic.min_frames);
                    object.lifetime_update = Some(
                        crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_delay_frames(
                            self.frame,
                            delay_frames,
                        ),
                    );
                }

                // C++ calls BodyModuleInterface::setInitialHealth with a
                // game-logic random percent after the object is created.
                let health_percent = gamelogic::helpers::get_game_logic_random_value_real(
                    generic.min_health,
                    generic.max_health,
                )
                .clamp(0.0, 1.0);
                object.health.current = object.health.maximum * health_percent;

                if generic.invulnerable_time > 0 {
                    object.apply_eject_invulnerable(
                        self.frame.saturating_add(generic.invulnerable_time),
                    );
                }
                if on_ground {
                    // `ON_GROUND_ALIGNED` gives every spawned object a random
                    // yaw after terrain placement (ObjectCreationList.cpp).
                    object.set_orientation(gamelogic::helpers::get_game_logic_random_value_real(
                        0.0,
                        std::f32::consts::TAU,
                    ));
                }
                // C++ ObjectCreationList.cpp:1086/1121-1124 ExtraFriction/Bounciness/BounceSound.
                object.set_extra_friction(generic.extra_friction);
                if generic.disposition.has(
                    gamelogic::object_creation_list::DebrisDisposition::SEND_IT_FLYING
                        | gamelogic::object_creation_list::DebrisDisposition::SEND_IT_UP
                        | gamelogic::object_creation_list::DebrisDisposition::RANDOM_FORCE,
                ) {
                    object.set_extra_bounciness(generic.extra_bounciness);
                    object.set_allow_bouncing(true);
                }
                if !generic.bounce_sound.is_empty() {
                    object.set_bounce_sound(generic.bounce_sound.clone());
                }
                if generic.fade_in {
                    object.start_drawable_fade_in(generic.fade_frames, self.frame);
                } else if generic.fade_out {
                    object.start_drawable_fade_out(generic.fade_frames, self.frame);
                }
                drop(object);
                if inherit_name {
                    if let Some(sid) = source_id {
                        // C++ ObjectCreationList.cpp:1005
                        // TheScriptEngine->transferObjectName(sourceObj->getName(), obj)
                        let _ = self.transfer_script_object_name(sid, created_id);
                    }
                }
                // C++ ObjectCreationList.cpp:962-969 — ParticleSystem is extra
                // attached fire/smoke. The object still spawns.
                if let Some(particle) = usable_ocl_particle_sys_name(&generic.particle_sys_name) {
                    let _ = self.combat_particles.attach_named_to_object(
                        created_id,
                        spawn_position,
                        self.frame,
                        particle,
                    );
                }
                created.push(created_id);
            }
        }
        created
    }

    /// Materialize FireFX and FireOCL events stamped when pending projectiles
    /// were drained. This is kept in GameLogic so both the normal host tick
    /// and GameWorld fire-spawn apply path use the same real object lifecycle.
    pub(crate) fn execute_pending_weapon_fire_ocls(&mut self) {
        let fire_ocls = self.combat_system.take_fire_ocl();
        for fire in fire_ocls {
            // C++ Weapon.cpp handles FireFX before invoking FireOCL.  Do not
            // synthesize a generic muzzle event for a blank FX reference: the
            // parsed name is authoritative and a null FXList is genuinely no
            // visual effect.
            if !fire.fire_fx_name.trim().is_empty()
                && !fire.fire_fx_name.trim().eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&fire.shooter_id));
                let matrix = self
                    .objects
                    .get(&fire.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = self
                    .combat_particles
                    .spawn_weapon_fire_fx_named_ocl_oriented(
                        fire.origin,
                        None,
                        self.frame,
                        fire.shooter_id,
                        None,
                        &fire.fire_fx_name,
                        "",
                        &fire.fire_ocl_name,
                        "",
                        0.0,
                        radius,
                        matrix,
                    );
            }
            if !fire.fire_ocl_name.trim().is_empty()
                && !fire.fire_ocl_name.trim().eq_ignore_ascii_case("None")
            {
                let _ = self.execute_parsed_weapon_ocl_at(
                    &fire.fire_ocl_name,
                    Some(fire.shooter_id),
                    fire.source_team,
                    fire.source_veterancy,
                    fire.source_orientation,
                    fire.source_velocity,
                    fire.origin,
                );
            }
        }
    }

    /// Flush real projectile impacts after either host- or GameWorld-owned
    /// flight integration.  Keeping this at the combat boundary ensures an
    /// OCL-only detonation cannot become a presentation-only no-op.
    pub(crate) fn flush_projectile_impact_fx(&mut self) {
        let impacts = self.combat_system.take_impact_fx();
        for impact in impacts {
            // C++ executes ProjectileDetonationFX and
            // ProjectileDetonationOCL independently.  An OCL-only weapon
            // (common poison/radiation field pattern) therefore still reaches
            // the real host object-creation path.
            if impact.detonation_fx_name.is_empty() && impact.detonation_ocl_name.is_empty() {
                continue;
            }
            if !impact.detonation_ocl_name.is_empty() {
                let _ = self.execute_parsed_weapon_ocl_at(
                    &impact.detonation_ocl_name,
                    Some(impact.shooter_id),
                    impact.source_team,
                    impact.source_veterancy,
                    impact.source_orientation,
                    impact.source_velocity,
                    impact.position,
                );
            }
            // C++ Weapon.cpp:903-939 plays ProjectileDetonationFX exactly once
            // (handleWeaponFireFX or a single doFXPos). Never invents a
            // fire-time MuzzleFlash at the crater and never unhides the
            // shooter's muzzle at detonation.
            if !impact.detonation_fx_name.trim().is_empty()
                && !impact
                    .detonation_fx_name
                    .trim()
                    .eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&impact.shooter_id));
                let matrix = self
                    .objects
                    .get(&impact.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = crate::game_logic::dispatch_fx_list_at_pos_oriented(
                    &impact.detonation_fx_name,
                    impact.position,
                    Some(impact.position),
                    0.0,
                    radius,
                    matrix,
                );
            }
        }
    }

    pub fn apply_nuke_cannon_primary_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        use crate::game_logic::host_nuke_cannon::{
            MEDIUM_RADIATION_AUDIO, NUKE_CANNON_DAMAGE_TYPE, NUKE_CANNON_DEATH_TYPE,
            NUKE_CANNON_FIRE_AUDIO, is_legal_nuke_cannon_splash_target,
            nuke_cannon_primary_damage_at, nuke_cannon_splash_radius,
        };

        let impact_xz = (impact.x, impact.z);
        let max_r = nuke_cannon_splash_radius();
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_nuke_cannon_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if dist <= max_r {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = nuke_cannon_primary_damage_at(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    NUKE_CANNON_DAMAGE_TYPE,
                    NUKE_CANNON_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.nuke_cannon_residual.record_primary_blast(hits);

        // Medium radiation field residual at impact.
        let source_id = source.unwrap_or(ObjectId(0));
        let _ = self.nuke_cannon_residual.spawn_radiation_zone(
            source_id,
            source_team,
            impact,
            self.frame,
        );
        self.queue_audio_event(
            AudioEventRequest::new(MEDIUM_RADIATION_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        self.queue_audio_event(
            AudioEventRequest::new(NUKE_CANNON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(160),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                None,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Overlord/Helix portable gattling residual at impact.
    ///
    /// Independent portable rider (C++ HelixContain.cpp:340 RIDERS ALWAYS FIRE):
    /// slot 1 = AA `GattlingBuildingGunAir`; slot 0 = ground `GattlingBuildingGun`
    /// only — never stacked onto OverlordTankGun / HelixMinigun host damage.
    pub(crate) fn apply_overlord_gattling_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
            OVERLORD_GATTLING_DEATH_TYPE, OVERLORD_GATTLING_FIRE_AUDIO,
            OVERLORD_GATTLING_GROUND_DAMAGE_TYPE, is_legal_overlord_gattling_target,
            overlord_gattling_ground_damage,
        };

        let chain = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_chain_guns_upgrade(&o.applied_upgrades))
            .unwrap_or(false);

        let (dmg, is_aa) = if slot == 1 {
            let mult = if chain { 1.25 } else { 1.0 };
            (OVERLORD_GATTLING_AIR_DAMAGE * mult, true)
        } else {
            (overlord_gattling_ground_damage(chain), false)
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
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
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| {
                        is_legal_overlord_gattling_target(
                            true,
                            false,
                            c.under_construction,
                            c.combat_kind,
                        )
                    },
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        if is_aa {
                            self.overlord_addons.record_gattling_aa_fire(0);
                        } else {
                            self.overlord_addons.record_gattling_ground_fire(0);
                        }
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            let combat_kind = obj.is_kind_of(KindOf::Attackable)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            if is_legal_overlord_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                combat_kind,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (
                        OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                } else {
                    (
                        OVERLORD_GATTLING_GROUND_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                };
                let destroyed =
                    obj.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        if is_aa {
            self.overlord_addons.record_gattling_aa_fire(hits);
        } else {
            self.overlord_addons.record_gattling_ground_fire(hits);
        }

        let muzzle = source
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(impact);
        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_GATTLING_FIRE_AUDIO)
                .with_position(muzzle)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                muzzle,
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// C++ HelixContain.cpp:340 — portable gattling auto-acquires independently
    /// of the host chassis shot (not a piggyback +10 on OverlordTankGun).
    pub(in super::super) fn try_overlord_gattling_addon_independent_fire(
        &mut self,
        host_id: ObjectId,
    ) {
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_RANGE, OVERLORD_GATTLING_GROUND_RANGE,
            is_legal_overlord_gattling_target, overlord_gattling_slot_for_air,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        let Some(host) = self.objects.get(&host_id) else {
            return;
        };
        if !host.has_overlord_gattling_residual() || !host.is_alive() {
            return;
        }
        if host.contained_by.is_some() {
            return;
        }
        let Some(aa) = host.weapon_slot(1) else {
            return;
        };
        if !Object::weapon_ready(aa, current_time) {
            return;
        }
        let team = host.team;
        let fire_pos = host.get_position();

        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter(|(id, _)| **id != host_id)
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
            host_id,
            team,
            fire_pos,
            candidates,
            |is_air| {
                if is_air {
                    OVERLORD_GATTLING_AIR_RANGE
                } else {
                    OVERLORD_GATTLING_GROUND_RANGE
                }
            },
            |c| {
                c.is_alive
                    && c.team != team
                    && !c.is_neutral
                    && is_legal_overlord_gattling_target(
                        c.is_alive,
                        false,
                        c.under_construction,
                        c.combat_kind,
                    )
            },
        );
        let Some((target_id, _, _)) = best else {
            return;
        };
        let target_is_air = self
            .objects
            .get(&target_id)
            .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
            .unwrap_or(false);
        let slot = overlord_gattling_slot_for_air(target_is_air);
        let impact = self
            .objects
            .get(&target_id)
            .map(|t| t.get_position())
            .unwrap_or(fire_pos);
        let (hits, _) =
            self.apply_overlord_gattling_residual_at(impact, Some(host_id), Some(target_id), slot);
        if hits == 0 {
            return;
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            if let Some(w) = host.weapon_slot_mut(1) {
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
        }
    }

    /// Residual honesty: Overlord/Helix gattling addon install + fire path.
    pub fn honesty_overlord_gattling_ok(&self) -> bool {
        self.overlord_addons.honesty_gattling_install_ok()
            && self.overlord_addons.honesty_gattling_fire_ok()
    }

    /// Residual honesty: Overlord/Helix/Emperor propaganda addon residual.
    pub fn honesty_overlord_propaganda_ok(&self) -> bool {
        self.overlord_addons.honesty_propaganda_install_ok() || self.honesty_propaganda_heal_ok()
    }

    /// Residual honesty: Helix transport residual.
    pub fn honesty_helix_transport_ok(&self) -> bool {
        self.overlord_addons.honesty_helix_transport_ok()
    }

    pub fn overlord_addons(
        &self,
    ) -> &crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry {
        &self.overlord_addons
    }

    /// Residual honesty: Nuke Cannon primary area + radiation residual.
    pub fn honesty_nuke_cannon_primary_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_primary_ok()
    }

    pub fn honesty_nuke_cannon_radiation_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_radiation_ok()
    }

    pub fn honesty_nuke_cannon_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_host_path_ok()
    }

    pub fn nuke_cannon_residual(
        &self,
    ) -> &crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry {
        &self.nuke_cannon_residual
    }

    /// Infer Technical salvage tier from residual weapon stats / upgrade tags.
    pub(in super::super) fn technical_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_technical::TechnicalWeaponTier {
        use crate::game_logic::host_technical::TechnicalWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("TechnicalCrateUpgradeTwo")
        {
            return TechnicalWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
            || obj.has_upgrade_tag("TechnicalCrateUpgradeOne")
        {
            return TechnicalWeaponTier::One;
        }
        // Infer from primary damage residual when tags absent.
        if let Some(w) = obj.weapon.as_ref() {
            if (w.damage - 50.0).abs() < 0.5 {
                return TechnicalWeaponTier::Two;
            }
            if (w.damage - 45.0).abs() < 0.5 {
                return TechnicalWeaponTier::One;
            }
        }
        TechnicalWeaponTier::Base
    }

    /// Apply residual salvage weapon tier to a Technical (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D gunner subobject swap.
    pub fn apply_technical_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_technical::TechnicalWeaponTier,
    ) -> bool {
        use crate::game_logic::host_technical::{
            TECHNICAL_TRANSPORT_SLOTS, delay_frames_to_reload_secs, is_technical_template,
            technical_weapon_for_tier, technical_weapon_name_for_tier, technical_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_technical_template(&obj.template_name) {
            return false;
        }

        // Ensure passenger residual capacity is installed.
        if !obj.is_technical_style_container() {
            obj.install_technical_transport();
        }
        let _ = TECHNICAL_TRANSPORT_SLOTS;

        let name = technical_weapon_name_for_tier(tier);
        let (dmg, range, min_range, delay, _splash) = technical_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| technical_weapon_for_tier(tier));
        // Force residual stats (store may lack min-range / reload).
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = min_range;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));
        obj.record_host_weapon_stats();

        // Tag residual crate upgrade for tier inference.
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_technical::TechnicalWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Base => {}
        }

        self.technical_residual_weapon_upgrades =
            self.technical_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Technical residual fire (MG direct or cannon/RPG splash).
    pub(in super::super) fn apply_technical_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_technical::{
            TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE, TECH_FIRE_AUDIO, TECH_MG_DAMAGE_TYPE,
            TECH_MG_DEATH_TYPE, TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE, TechnicalWeaponTier,
            is_legal_technical_splash_target, is_technical_template, technical_cannon_scatter_aim,
            technical_cannon_scatter_misses_infantry, technical_splash_damage_at,
            technical_weapon_stats,
        };

        let tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::technical_tier_from_object)
            .unwrap_or(TechnicalWeaponTier::Base);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        let (_dmg, _range, _min, _delay, splash) = technical_weapon_stats(tier);

        // C++ TechnicalCannonWeapon ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let cannon_tier =
            matches!(tier, TechnicalWeaponTier::One | TechnicalWeaponTier::Two) && splash > 0.0;
        if intended_is_infantry && cannon_tier {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = technical_cannon_scatter_aim(impact, true, seed);
            if scattered {
                self.technical_cannon_scatter_applied =
                    self.technical_cannon_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if technical_cannon_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > splash {
                        self.technical_cannon_scatter_misses =
                            self.technical_cannon_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        // Collect candidates: intended always; splash ring when tier has radius.
        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_technical_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended
                    && intended_is_infantry
                    && cannon_tier
                    && (splash <= 0.0 || dist > splash)
                {
                    return None;
                }
                if is_intended || (splash > 0.0 && dist <= splash) {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = technical_splash_damage_at(tier, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = match tier {
                    TechnicalWeaponTier::Base => (TECH_MG_DAMAGE_TYPE, TECH_MG_DEATH_TYPE),
                    TechnicalWeaponTier::One => (TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE),
                    TechnicalWeaponTier::Two => (TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE),
                };
                let destroyed =
                    obj.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.technical_residual_fires = self.technical_residual_fires.saturating_add(1);
        self.technical_residual_units_hit = self.technical_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TECH_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_technical_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Record Technical residual passenger load honesty (Enter residual).
    pub fn record_technical_residual_load(&mut self) {
        self.technical_residual_loads = self.technical_residual_loads.saturating_add(1);
    }

    /// Record Technical residual passenger unload honesty.
    pub fn record_technical_residual_unload(&mut self) {
        self.technical_residual_unloads = self.technical_residual_unloads.saturating_add(1);
    }

    /// Infer Marauder salvage tier from residual weapon reload / upgrade tags.
    pub(in super::super) fn marauder_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_marauder::MarauderWeaponTier {
        use crate::game_logic::host_marauder::MarauderWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("MarauderCrateUpgradeTwo")
        {
            return MarauderWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
            || obj.has_upgrade_tag("MarauderCrateUpgradeOne")
        {
            return MarauderWeaponTier::One;
        }
        // Infer from reload residual when tags absent (faster = higher tier).
        if let Some(w) = obj.weapon.as_ref() {
            // Tier2 ~23/30s, Tier1 ~45/30s, Base ~60/30s.
            if w.reload_time <= (23.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::Two;
            }
            if w.reload_time <= (45.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::One;
            }
        }
        MarauderWeaponTier::Base
    }

    /// Apply BlackNapalm residual to an Inferno Cannon (PLAYER_UPGRADE fire field residual).
    ///
    /// Tags the unit so subsequent shell impacts spawn FireFieldUpgradedSmall
    /// residual (7.5 dmg/tick). Fail-closed: not HistoricBonus Firestorm matrix.
    pub fn apply_inferno_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::UPGRADE_CHINA_BLACK_NAPALM;
        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_inferno_cannon_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        self.inferno_black_napalm_residual_upgrades = self
            .inferno_black_napalm_residual_upgrades
            .saturating_add(1);
        true
    }

    /// Apply BlackNapalm residual to a Dragon Tank (PLAYER_UPGRADE flame residual).
    pub fn apply_dragon_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::{
            UPGRADE_CHINA_BLACK_NAPALM, dragon_flame_weapon, is_dragon_tank_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_dragon_tank_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        let _ = obj.replace_weapon_set_slot(0, Some(dragon_flame_weapon(true)));
        self.dragon_tank_residual_black_napalm_upgrades = self
            .dragon_tank_residual_black_napalm_upgrades
            .saturating_add(1);
        true
    }

    /// Apply Chain Guns residual to a Gattling Tank or Gattling Cannon structure
    /// (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_gattling_chain_guns_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_base_defense::{
            gattling_building_air_weapon, gattling_building_ground_weapon,
            is_gattling_cannon_structure,
        };
        use crate::game_logic::host_gattling_tank::{
            GattlingFireLevel, UPGRADE_CHINA_CHAIN_GUNS, gattling_air_weapon,
            gattling_ground_weapon, is_gattling_tank_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let is_tank = is_gattling_tank_template(&obj.template_name);
        let is_building = is_gattling_cannon_structure(&obj.template_name);
        if !is_tank && !is_building {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        let level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        if is_tank {
            // Chain Guns is a C++ WeaponBonusUpgrade, so it must retain the
            // live weapon/barrel state rather than rebind the WeaponSet.
            obj.weapon = Some(gattling_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_air_weapon(level, true));
            self.gattling_tank_residual_chain_gun_upgrades = self
                .gattling_tank_residual_chain_gun_upgrades
                .saturating_add(1);
        } else {
            obj.weapon = Some(gattling_building_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_building_air_weapon(level, true));
            self.gattling_building_residual_chain_gun_upgrades = self
                .gattling_building_residual_chain_gun_upgrades
                .saturating_add(1);
        }
        true
    }

    /// Advance structure Gattling Cannon continuous-fire ramp residual after a shot.
    pub(in super::super) fn advance_gattling_building_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_base_defense::{
            GATTLING_BUILDING_RAPID_FIRE_AUDIO, gattling_building_air_weapon,
            gattling_building_coast_until_after_shot, gattling_building_ground_weapon,
            gattling_building_has_chain_guns, gattling_building_on_shot_fired,
        };
        use crate::game_logic::host_gattling_tank::GattlingFireLevel;

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.continuous_fire_consecutive;
        let prev_victim = if obj.continuous_fire_victim == 0 {
            None
        } else {
            Some(obj.continuous_fire_victim)
        };
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_building_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = gattling_building_has_chain_guns(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.record_host_continuous_fire();
        obj.continuous_fire_consecutive = consecutive;
        obj.continuous_fire_victim = new_victim.unwrap_or(0);
        obj.continuous_fire_coast_until_frame =
            gattling_building_coast_until_after_shot(frame, new_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_building_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        obj.record_host_weapon_stats();
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_building_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }
        obj.record_host_weapon_stats();

        if slot == 1 {
            self.gattling_building_residual_aa_fires =
                self.gattling_building_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_building_residual_ground_fires = self
                .gattling_building_residual_ground_fires
                .saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_building_residual_ramp_mean =
                self.gattling_building_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_building_residual_ramp_fast =
                self.gattling_building_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_BUILDING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Advance Gattling continuous-fire ramp residual after a successful shot.
    pub(in super::super) fn advance_gattling_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_gattling_tank::{
            GATTLING_RAPID_FIRE_AUDIO, GattlingFireLevel, gattling_air_weapon,
            gattling_coast_until_after_shot, gattling_ground_weapon, gattling_on_shot_fired,
            has_chain_guns_upgrade,
        };

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.continuous_fire_consecutive;
        let prev_victim = if obj.continuous_fire_victim == 0 {
            None
        } else {
            Some(obj.continuous_fire_victim)
        };
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = has_chain_guns_upgrade(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.record_host_continuous_fire();
        obj.continuous_fire_consecutive = consecutive;
        obj.continuous_fire_victim = new_victim.unwrap_or(0);
        obj.continuous_fire_coast_until_frame = gattling_coast_until_after_shot(frame, new_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        obj.record_host_weapon_stats();
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }
        obj.record_host_weapon_stats();

        if slot == 1 {
            self.gattling_tank_residual_aa_fires =
                self.gattling_tank_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_tank_residual_ground_fires =
                self.gattling_tank_residual_ground_fires.saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_tank_residual_ramp_mean =
                self.gattling_tank_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_tank_residual_ramp_fast =
                self.gattling_tank_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Apply Dragon Tank flame residual at impact (primary + secondary splash).
    ///
    /// Returns (units_hit, any_destroyed).
    pub(in super::super) fn apply_dragon_flame_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_DAMAGE_TYPE, DRAGON_DEATH_TYPE, DRAGON_FIRE_AUDIO, DRAGON_SECONDARY_RADIUS,
            dragon_flame_damage_at, has_black_napalm_upgrade, is_legal_dragon_flame_target,
        };

        let upgraded = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_black_napalm_upgrade(&o.applied_upgrades))
            .unwrap_or(false);
        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_dragon_flame_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                if is_intended || dist <= DRAGON_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = dragon_flame_damage_at(upgraded, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    DRAGON_DAMAGE_TYPE,
                    DRAGON_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.dragon_tank_residual_fires = self.dragon_tank_residual_fires.saturating_add(1);
        self.dragon_tank_residual_units_hit =
            self.dragon_tank_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(DRAGON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Gattling Tank residual hit at impact (single-target residual).
    ///
    /// Returns (units_hit, any_destroyed). Continuous-fire ramp advances in
    /// `advance_gattling_continuous_fire` after the fire path records last_fire_time.
    pub(in super::super) fn apply_gattling_tank_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::{
            GATTLING_AIR_DAMAGE, GATTLING_AIR_DAMAGE_TYPE, GATTLING_DEATH_TYPE,
            GATTLING_FIRE_AUDIO, GATTLING_GROUND_DAMAGE, GATTLING_GROUND_DAMAGE_TYPE,
            gattling_damage_with_chain_guns, has_chain_guns_upgrade, is_legal_gattling_target,
        };

        let (base_dmg, chain) = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| {
                let chain = has_chain_guns_upgrade(&o.applied_upgrades);
                let base = if slot == 1 {
                    GATTLING_AIR_DAMAGE
                } else {
                    GATTLING_GROUND_DAMAGE
                };
                (base, chain)
            })
            .unwrap_or((GATTLING_GROUND_DAMAGE, false));
        let dmg = gattling_damage_with_chain_guns(base_dmg, chain);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
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
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| is_legal_gattling_target(true, false, c.under_construction, c.combat_kind),
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        self.queue_audio_event(
                            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                                .with_position(impact)
                                .with_priority(140),
                        );
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            if is_legal_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                true,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (GATTLING_AIR_DAMAGE_TYPE, GATTLING_DEATH_TYPE)
                } else {
                    (GATTLING_GROUND_DAMAGE_TYPE, GATTLING_DEATH_TYPE)
                };
                let destroyed =
                    obj.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.queue_audio_event(
            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                Some(tid),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply residual salvage fire-rate tier to a Marauder (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D turret subobject swap.
    pub fn apply_marauder_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_marauder::MarauderWeaponTier,
    ) -> bool {
        use crate::game_logic::host_marauder::{
            delay_frames_to_reload_secs, is_marauder_template, marauder_weapon_for_tier,
            marauder_weapon_name_for_tier, marauder_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_marauder_template(&obj.template_name) {
            return false;
        }

        let name = marauder_weapon_name_for_tier(tier);
        let (dmg, range, delay, _splash, speed) = marauder_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| marauder_weapon_for_tier(tier));
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = 0.0;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.projectile_speed = speed;
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));
        obj.record_host_weapon_stats();

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_marauder::MarauderWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Base => {}
        }

        self.marauder_residual_weapon_upgrades =
            self.marauder_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Marauder residual fire (primary on intended + small splash radius).
    /// C++ MarauderTankShell DumbProjectile residual.
    pub fn spawn_marauder_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        weapon_speed: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_marauder::{
            MARAUDER_SHELL_MAX_HEALTH, MARAUDER_TANK_SHELL, marauder_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(MARAUDER_TANK_SHELL) {
            let mut t = ThingTemplate::new(MARAUDER_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(MARAUDER_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(MARAUDER_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on MarauderTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_marauder::marauder_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_marauder::marauder_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_marauder::MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(MARAUDER_TANK_SHELL, team, start)?;
        let frames = marauder_shell_flight_frames(start, aim, weapon_speed).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.marauder_shell_projectile = true;
            o.marauder_shell_from = Some([start.x, start.y, start.z]);
            o.marauder_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.marauder_shell_launch_frame = Some(self.frame);
            o.marauder_shell_flight_frames = frames;
            o.marauder_shell_intended = intended.map(|id| id.0);
            o.marauder_shell_weapon_speed = weapon_speed;
            o.note_producer(source_id);
            o.health.maximum = MARAUDER_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, MARAUDER_SHELL_MAX_HEALTH);
        }
        self.marauder_shells_spawned = self.marauder_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_marauder_shell_projectiles(&mut self) {
        use crate::game_logic::host_marauder::marauder_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.marauder_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .marauder_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .marauder_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.marauder_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.marauder_shell_launch_frame.unwrap_or(frame),
                    o.marauder_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = marauder_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
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
                o.marauder_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_marauder_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_marauder_shell_projectile_ok(&self) -> bool {
        self.marauder_shells_spawned > 0
    }

    pub fn apply_marauder_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_marauder::{
            MARAUDER_DAMAGE_TYPE, MARAUDER_DEATH_TYPE, MARAUDER_FIRE_AUDIO, MARAUDER_SPLASH_RADIUS,
            is_legal_marauder_splash_target, is_marauder_template, marauder_scatter_aim,
            marauder_scatter_misses_infantry, marauder_splash_damage_at,
        };

        // Fire-rate tier residual is encoded on the weapon; damage is constant across tiers.
        let _tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::marauder_tier_from_object);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ MarauderTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = marauder_scatter_aim(impact, true, seed);
            if scattered {
                self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if marauder_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_marauder_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended && intended_is_infantry && dist > MARAUDER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= MARAUDER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = marauder_splash_damage_at(is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    MARAUDER_DAMAGE_TYPE,
                    MARAUDER_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.marauder_residual_fires = self.marauder_residual_fires.saturating_add(1);
        self.marauder_residual_units_hit = self.marauder_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(MARAUDER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_marauder_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Scorpion salvage gun tier residual (primary damage 20 → 25).
    pub fn apply_scorpion_salvage_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_scorpion::ScorpionSalvageTier,
    ) -> bool {
        use crate::game_logic::host_scorpion::{
            ScorpionSalvageTier, has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
            is_scorpion_template, scorpion_gun_weapon, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            ScorpionSalvageTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            ScorpionSalvageTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            ScorpionSalvageTier::Base => {}
        }

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut gun = scorpion_gun_weapon(tier);
        gun.last_fire_time = last_fire;
        let _ = obj.replace_weapon_set_slot(0, Some(gun));

        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(ap, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // Salvage changes the selected C++ WeaponSet.  Even when AP
            // Rockets also contributes its in-place stats, this concrete
            // secondary Weapon instance was rebuilt and starts with a fresh
            // barrel cursor.
            let _ = obj.replace_weapon_set_slot(1, Some(missile));
        }

        self.scorpion_residual_salvage_upgrades =
            self.scorpion_residual_salvage_upgrades.saturating_add(1);
        true
    }

    /// Equip Scorpion Rocket secondary residual (Upgrade_GLAScorpionRocket).
    pub fn apply_scorpion_rocket_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_SCORPION_ROCKET, has_ap_rockets_upgrade, is_scorpion_template,
            salvage_tier_from_upgrades, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_SCORPION_ROCKET.to_string());
        let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
        let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
        let _ = obj.replace_weapon_set_slot(
            1,
            Some(scorpion_missile_weapon(ap, tier.dual_missile_clip())),
        );
        self.scorpion_residual_rocket_upgrades =
            self.scorpion_residual_rocket_upgrades.saturating_add(1);
        true
    }

    /// Apply AP Rockets residual damage mult to Scorpion missile secondary.
    pub fn apply_scorpion_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_AP_ROCKETS, has_scorpion_rocket_upgrade, is_scorpion_template,
            salvage_tier_from_upgrades, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(true, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // AP Rockets is a C++ WeaponBonus upgrade layered onto the
            // selected Scorpion Rocket WeaponSet; preserve its live barrel
            // cursor while refreshing the boosted stats.
            obj.secondary_weapon = Some(missile);
        }
        true
    }

    /// C++ ScorpionTankShell DumbProjectile residual.
    pub fn spawn_scorpion_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            SCORPION_SHELL_MAX_HEALTH, SCORPION_TANK_SHELL, scorpion_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_TANK_SHELL) {
            let mut t = ThingTemplate::new(SCORPION_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_GUN_SPLASH_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(SCORPION_TANK_SHELL, team, start)?;
        let frames = scorpion_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_shell_projectile = true;
            o.scorpion_shell_from = Some([start.x, start.y, start.z]);
            o.scorpion_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_shell_launch_frame = Some(self.frame);
            o.scorpion_shell_flight_frames = frames;
            o.scorpion_shell_slot = slot;
            o.note_producer(source_id);
            o.health.maximum = SCORPION_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, SCORPION_SHELL_MAX_HEALTH);
        }
        self.scorpion_shells_spawned = self.scorpion_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_scorpion_shell_projectiles(&mut self) {
        use crate::game_logic::host_scorpion::scorpion_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.scorpion_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, u8)> = Vec::new();
        for id in flying {
            let (source, from, aim, launch, frames, slot) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .scorpion_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .scorpion_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    from,
                    aim,
                    o.scorpion_shell_launch_frame.unwrap_or(frame),
                    o.scorpion_shell_flight_frames.max(1),
                    o.scorpion_shell_slot,
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = scorpion_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, aim, slot));
            }
        }
        for (id, source, pos, slot) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
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
                o.scorpion_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_scorpion_residual_at(pos, source, None, slot);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// C++ ScorpionMissile ProjectileObject residual.
    pub fn spawn_scorpion_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            SCORPION_MISSILE, SCORPION_MISSILE_FUEL_FRAMES, SCORPION_MISSILE_INITIAL_VELOCITY,
            SCORPION_MISSILE_MAX_HEALTH, SCORPION_MISSILE_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_MISSILE) {
            let mut t = ThingTemplate::new(SCORPION_MISSILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_MISSILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionMissileWeapon vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_MISSILE_SECONDARY_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object(SCORPION_MISSILE, team, start)?;
        let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_missile_projectile = true;
            o.scorpion_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_missile_intended = intended.map(|id| id.0);
            o.scorpion_missile_travelled = 0.0;
            o.scorpion_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(SCORPION_MISSILE_FUEL_FRAMES));
            o.scorpion_missile_slot = slot;
            o.note_producer(source_id);
            o.health.maximum = SCORPION_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, SCORPION_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = SCORPION_MISSILE_PROJECTILE_SPEED; // cruise used in update
        self.scorpion_missiles_spawned = self.scorpion_missiles_spawned.saturating_add(1);
        Some(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ThingTemplate;
    use std::path::PathBuf;

    #[test]
    fn retail_fire_field_ocl_creates_the_authored_object_for_the_source_team() {
        // Load the actual retail source rather than registering an invented
        // OCL. This exercises the same parsed store lookup used by a Weapon
        // FireOCL/ProjectileDetonationOCL in a live game.
        // extracted_big_files_v2 is the populated extraction on this repo
        // (v1 is absent); fall back to v1 where it exists.
        let mut retail = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("windows_game/extracted_big_files_v2/INI/ObjectCreationList.ini");
        if !retail.is_file() {
            retail = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini");
        }
        gamelogic::object_creation_list::store::load_object_creation_lists_from_path(&retail)
            .expect("retail ObjectCreationList.ini must parse");

        let mut logic = GameLogic::new();
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        // The test supplies the exact existing Object identity; production
        // obtains it from the parsed asset definition before creating it.
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object("__OclBridgeSource", Team::China, Vec3::new(10.0, 3.0, 20.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_FireFieldSmall",
            Some(source),
            Team::China,
            VeterancyLevel::Veteran,
            0.0,
            Vec3::ZERO,
            Vec3::new(10.0, 3.0, 20.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
        assert_eq!(field.producer_id, Some(source));
        assert_eq!(field.get_position(), Vec3::new(10.0, 3.0, 20.0));
    }

    #[test]
    fn mixed_ocl_still_creates_supported_nuggets_when_one_needs_leftover_physics() {
        // C++ ObjectCreationList.cpp:1524-1534 createInternal runs every nugget.
        // A leftover-physics CreateDebris must not drop the CreateObject spawn.
        let ini = r#"
ObjectCreationList OCL_HostMixedParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
  CreateDebris
    ModelNames = EXRockChunk
    Count = 1
    Disposition = SEND_IT_FLYING
    ExtraFriction = -0.3
  End
End
"#;
        gamelogic::object_creation_list::store::load_object_creation_lists_from_str(ini)
            .expect("mixed OCL must parse");

        let mut logic = GameLogic::new();
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object("__OclBridgeSource", Team::China, Vec3::new(4.0, 1.0, 8.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostMixedParity",
            Some(source),
            Team::China,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(4.0, 1.0, 8.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
    }

    /// C++ ObjectCreationList.cpp:1302-1305 — OCL create uses the source
    /// controlling player, not `unique_player_id_for_team` (None in 2v2).
    #[test]
    fn weapon_ocl_create_uses_source_owner_player_id_in_2v2() {
        let ini = r#"
ObjectCreationList OCL_HostOwnerParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
End
"#;
        gamelogic::object_creation_list::store::load_object_creation_lists_from_str(ini)
            .expect("owner OCL must parse");

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(3, Team::China, "ChinaA", true));
        logic.add_player(Player::new(7, Team::China, "ChinaB", false));
        assert_eq!(logic.unique_player_id_for_team(Team::China), None);

        logic.templates.insert(
            "__OclOwnerSource".into(),
            ThingTemplate::new("__OclOwnerSource"),
        );
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object_for_player("__OclOwnerSource", 7, Vec3::new(10.0, 3.0, 20.0))
            .expect("source object");
        assert_eq!(
            logic.objects.get(&source).and_then(|o| o.owner_player_id),
            Some(7)
        );

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostOwnerParity",
            Some(source),
            Team::China,
            VeterancyLevel::Veteran,
            0.0,
            Vec3::ZERO,
            Vec3::new(10.0, 3.0, 20.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
        assert_eq!(field.owner_player_id, Some(7));
        assert_eq!(field.producer_id, Some(source));
    }

    /// C++ ObjectCreationList.cpp:996-1005 — inherit only if isTrainable
    /// and transferObjectName with the source script name.
    #[test]
    fn ocl_inherit_veterancy_only_if_trainable_and_transfers_script_name() {
        let ini = r#"
ObjectCreationList OCL_HostInheritVetParity
  CreateObject
    ObjectNames = OclEjectedPilot
    Count = 1
    InheritsVeterancy = Yes
  End
End
ObjectCreationList OCL_HostInheritFieldParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
    InheritsVeterancy = Yes
  End
End
"#;
        gamelogic::object_creation_list::store::load_object_creation_lists_from_str(ini)
            .expect("inherit OCL must parse");

        let mut logic = GameLogic::new();
        let mut source_tpl = ThingTemplate::new("__OclVetSource");
        source_tpl.is_trainable = true;
        logic.templates.insert("__OclVetSource".into(), source_tpl);

        let mut pilot = ThingTemplate::new("OclEjectedPilot");
        pilot.is_trainable = true;
        logic.templates.insert("OclEjectedPilot".into(), pilot);
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );

        let source = logic
            .create_object("__OclVetSource", Team::USA, Vec3::new(2.0, 0.0, 4.0))
            .expect("source");
        {
            let src = logic.objects.get_mut(&source).expect("source mut");
            src.name = "PilotOne".into();
            src.set_min_veterancy_level(VeterancyLevel::Elite);
        }

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostInheritVetParity",
            Some(source),
            Team::USA,
            VeterancyLevel::Elite,
            0.0,
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 4.0),
        );
        assert_eq!(created.len(), 1);
        let pilot_id = created[0];
        let spawned = logic.objects.get(&pilot_id).expect("pilot");
        assert_eq!(spawned.experience.level, VeterancyLevel::Elite);
        assert_eq!(spawned.name, "PilotOne");
        assert_eq!(
            logic.objects.get(&source).map(|o| o.name.as_str()),
            Some("")
        );
        assert_eq!(logic.find_object_id_by_name("PilotOne"), Some(pilot_id));

        let field_src = logic
            .create_object("__OclVetSource", Team::USA, Vec3::new(8.0, 0.0, 4.0))
            .expect("field source");
        {
            let src = logic.objects.get_mut(&field_src).expect("field src mut");
            src.name = "FieldSrc".into();
            src.set_min_veterancy_level(VeterancyLevel::Elite);
        }
        let created_field = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostInheritFieldParity",
            Some(field_src),
            Team::USA,
            VeterancyLevel::Elite,
            0.0,
            Vec3::ZERO,
            Vec3::new(8.0, 0.0, 4.0),
        );
        assert_eq!(created_field.len(), 1);
        let field = logic.objects.get(&created_field[0]).expect("field");
        assert!(!field.is_trainable());
        assert_eq!(field.experience.level, VeterancyLevel::Rookie);
        assert!(field.name.is_empty());
        assert_eq!(
            logic.objects.get(&field_src).map(|o| o.name.as_str()),
            Some("FieldSrc")
        );
    }

    /// C++ HelixContain.cpp:340 — portable gattling fires without host shot.
    #[test]
    fn overlord_gattling_independent_acquire_not_stacked_primary() {
        use crate::game_logic::{AIState, KindOf, Team};
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
            o.install_overlord_gattling_addon();
            o.set_ai_state(AIState::Idle);
            o.target = None;
            if let Some(w) = o.weapon_slot_mut(1) {
                w.last_fire_time = -10.0;
                w.reload_time = 0.1;
            }
        }
        let victim = logic
            .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("victim");
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_overlord_gattling_addon_independent_fire(tank);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        let dealt = hp_before - hp_after;
        assert!(
            (dealt - crate::game_logic::host_overlord_addons::OVERLORD_GATTLING_GROUND_DAMAGE)
                .abs()
                < 0.2,
            "independent gattling deals 10 not stacked primary+10, dealt={dealt}"
        );
        assert!(logic.overlord_addons.gattling_ground_fires > 0);
    }

    #[test]
    fn fade_in_nugget_spawns_and_starts_drawable_fade() {
        let ini = r#"
ObjectCreationList OCL_HostFadeInParity
  CreateObject
    ObjectNames = FadeDebris
    Count = 1
    FadeIn = Yes
    FadeTime = 1000
  End
End
"#;
        gamelogic::object_creation_list::store::load_object_creation_lists_from_str(ini)
            .expect("fade OCL must parse");

        let mut logic = GameLogic::new();
        logic.frame = 40;
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        logic
            .templates
            .insert("FadeDebris".into(), ThingTemplate::new("FadeDebris"));
        let source = logic
            .create_object("__OclBridgeSource", Team::USA, Vec3::new(1.0, 0.0, 1.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostFadeInParity",
            Some(source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 1.0),
        );
        assert_eq!(created.len(), 1);
        let debris = logic.objects.get(&created[0]).expect("faded debris");
        assert_eq!(
            debris.drawable_fade_mode,
            crate::game_logic::DRAWABLE_FADE_IN
        );
        assert_eq!(debris.drawable_fade_start_frame, 40);
        assert!(debris.drawable_fade_frames > 0);
        assert!((debris.drawable_fade_opacity(40) - 0.0).abs() < 1e-5);
        assert!(debris.drawable_fade_opacity(40 + debris.drawable_fade_frames) > 0.99);
    }
}
