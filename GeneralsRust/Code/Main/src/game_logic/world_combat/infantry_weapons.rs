//! Host combat `impl GameLogic` — `infantry_weapons`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn apply_scud_area_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
        toxin_warhead: bool,
    ) -> (u32, bool) {
        use crate::game_logic::host_scud_launcher::{
            SCUD_DAMAGE_TYPE, SCUD_DEATH_TYPE, SCUD_FIRE_AUDIO, SCUD_POISON_AUDIO,
            SCUD_POISON_DAMAGE_TYPE, SCUD_POISON_DEATH_TYPE, UPGRADE_GLA_ANTHRAX_BETA,
            is_legal_scud_splash_target, scud_explosive_damage_at, scud_splash_radius,
            scud_toxin_blast_damage_at,
        };

        let impact_xz = (impact.x, impact.z);
        let max_r = scud_splash_radius(toxin_warhead);
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
                if !is_legal_scud_splash_target(
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
            let dmg = if toxin_warhead {
                scud_toxin_blast_damage_at(dist)
            } else {
                scud_explosive_damage_at(dist)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = if toxin_warhead {
                    (SCUD_POISON_DAMAGE_TYPE, SCUD_POISON_DEATH_TYPE)
                } else {
                    (SCUD_DAMAGE_TYPE, SCUD_DEATH_TYPE)
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

        self.scud_poison_zones.record_area_blast(hits);

        // Toxin / Anthrax residual: spawn MediumPoisonField DoT (Beta/Gamma tier).
        if toxin_warhead {
            use crate::game_logic::host_toxin_tractor::{
                AnthraxResidualTier, UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                anthrax_tier_from_flags, is_chem_general_template,
            };
            let anthrax = source
                .and_then(|sid| {
                    self.objects.get(&sid).map(|a| {
                        let has_gamma = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                            || a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                            || a.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || a.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                        let has_beta = a.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || a.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                        anthrax_tier_from_flags(
                            has_gamma,
                            has_beta,
                            is_chem_general_template(&a.template_name),
                        )
                    })
                })
                .unwrap_or(AnthraxResidualTier::None);
            let source_id = source.unwrap_or(ObjectId(0));
            let _ = self.scud_poison_zones.spawn_zone(
                source_id,
                source_team,
                impact,
                self.frame,
                anthrax,
            );
            self.queue_audio_event(
                AudioEventRequest::new(SCUD_POISON_AUDIO)
                    .with_position(impact)
                    .with_priority(140),
            );
        }

        self.queue_audio_event(
            AudioEventRequest::new(SCUD_FIRE_AUDIO)
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

    /// Apply residual multi-barrel salvage tier to a Quad Cannon (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D turret subobject swap.
    pub fn apply_quad_cannon_barrel_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_quad_cannon::QuadCannonBarrelTier,
    ) -> bool {
        use crate::game_logic::host_quad_cannon::{
            delay_frames_to_reload_secs, is_quad_cannon_template, quad_air_stats,
            quad_ground_stats, quad_weapon_names_for_tier,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_quad_cannon_template(&obj.template_name) {
            return false;
        }

        let (g_name, a_name) = quad_weapon_names_for_tier(tier);
        let (g_dmg, g_range, g_delay) = quad_ground_stats(tier);
        let (a_dmg, a_range, a_delay) = quad_air_stats(tier);

        // Prefer store weapons when present; otherwise apply stats residual directly.
        let ground = ThingTemplate::weapon_from_store(g_name).unwrap_or_else(|| {
            let mut w = crate::game_logic::Weapon::default();
            w.damage = g_dmg;
            w.range = g_range;
            w.reload_time = delay_frames_to_reload_secs(g_delay);
            w.can_target_air = false;
            w.can_target_ground = true;
            w.projectile_speed = 0.0;
            w
        });
        let mut air = ThingTemplate::weapon_from_store(a_name).unwrap_or_else(|| {
            let mut w = crate::game_logic::Weapon::default();
            w.damage = a_dmg;
            w.range = a_range;
            w.reload_time = delay_frames_to_reload_secs(a_delay);
            w.can_target_air = true;
            w.can_target_ground = false;
            w.projectile_speed = 0.0;
            w
        });
        // Ensure AA residual flags even if store template lacked anti mask.
        air.can_target_air = true;
        air.can_target_ground = false;

        // C++ replaces the concrete PRIMARY Weapon when a salvage tier
        // applies.  Its mutable barrel cursor belongs to that instance, not
        // to the template-name slot, so do not carry a partially consumed
        // cursor into the residual replacement.
        let _ = obj.replace_weapon_set_slot(0, Some(ground));
        obj.record_host_weapon_stats();
        let _ = obj.replace_weapon_set_slot(1, Some(air));
        obj.record_host_weapon_stats();
        self.quad_cannon_residual_barrel_upgrades =
            self.quad_cannon_residual_barrel_upgrades.saturating_add(1);
        true
    }

    /// Advance SCUD MediumPoisonField residual zones.
    /// C++ TensileFormationUpdate residual (AvalancheChunk springy slide).
    /// C++ TensileFormationUpdate residual (AvalancheChunk springy slide).
    /// Test/host helper: ignite a flammable object residual.
    pub fn ignite_object_fire_spread(&mut self, id: ObjectId) -> bool {
        let frame = self.frame as u32;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if !obj.try_ignite_fire_spread(frame) {
            return false;
        }
        self.fire_spread_reg.record_ignition();
        let pos = obj.get_position();
        let sound = obj
            .fire_spread
            .as_ref()
            .map(|f| f.burning_sound_name.clone())
            .unwrap_or_default();
        self.start_fire_spread_burning_sound(id, pos, &sound);
        self.spawn_auto_aflame_particles(id, pos);
        true
    }

    /// C++ FireSpreadUpdate + FlammableUpdate residual (tree fire chain).
    /// C++ FireSpreadUpdate + FlammableUpdate residual (tree fire chain).
    /// C++ BaseRegenerateUpdate residual (structure auto-heal after delay).
    /// C++ EnemyNearUpdate residual (scan vision for enemies → ENEMYNEAR).
    /// C++ AnimationSteeringUpdate residual (Battle Bus turn model conditions).
    /// Record ProneUpdate goProne residual after damage (host helper).
    pub fn record_prone_go_if_needed(&mut self, id: ObjectId, damage: f32) {
        if let Some(obj) = self.objects.get_mut(&id) {
            if let Some(pu) = obj.prone_update.as_mut() {
                let before = pu.prone_frames;
                if pu.go_prone_damage(damage) || pu.prone_frames > before {
                    let added = (pu.prone_frames - before).max(0) as u32;
                    self.prone_update_reg.record_go_prone(added);
                }
            }
        }
    }

    /// C++ FloatUpdate residual (boat sway / optional water snap).
    pub(in super::super) fn update_float_update(&mut self) {
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.float_update.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let water_y = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                let pos = obj.get_position();
                self.terrain
                    .as_ref()
                    .and_then(|t| t.water_surface_at_world(pos))
                    .or_else(|| {
                        crate::game_logic::host_float_update::leftover_water_surface_y(pos.x, pos.z)
                    })
            };
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(fu) = obj.float_update.as_mut() else {
                continue;
            };
            fu.tick_sway(frame);
            crate::game_logic::host_float_update::publish_sway(id.0, fu.yaw, fu.pitch);
            self.float_update_reg.record_sway();
            if let Some(wy) = fu.snap_height_y(water_y) {
                let mut p = obj.get_position();
                p.y = wy;
                obj.set_position(p);
                self.float_update_reg.record_snap();
            }
        }
    }

    /// C++ ProneUpdate residual countdown + NO_ATTACK / PRONE condition.
    /// C++ OCL DeliveryDecal residual: create radius decal on SW host.
    pub fn create_delivery_radius_decal(&mut self, host_id: ObjectId, target_pos: Vec3) -> bool {
        self.create_delivery_radius_decal_with_radius(host_id, target_pos, 0.0)
    }

    /// `radius <= 0` uses the host-template default (Scud/nuke). Cargo flights pass the OCL peel.
    pub fn create_delivery_radius_decal_with_radius(
        &mut self,
        host_id: ObjectId,
        target_pos: Vec3,
        radius: f32,
    ) -> bool {
        let frame = self.frame as u32;
        let Some(obj) = self.objects.get_mut(&host_id) else {
            return false;
        };
        let ok = if radius > 0.0 {
            obj.create_delivery_radius_decal_with_radius(target_pos, frame, radius)
        } else {
            obj.create_delivery_radius_decal(target_pos, frame)
        };
        if ok {
            self.radius_decal_update_reg.record_create();
            obj.status.attacking = true;
            true
        } else {
            false
        }
    }

    /// C++ RadiusDecalUpdate::update residual.
    /// C++ CheckpointUpdate residual (open gate for allies when clear of enemies).
    /// C++ SpectreGunshipDeploymentUpdate::initiateIntent residual.
    /// C++ SpectreGunshipDeploymentUpdate::initiateIntent residual.
    pub fn initiate_spectre_gunship_deployment(
        &mut self,
        caster_id: ObjectId,
        target_pos: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_spectre_gunship_deployment::default_map_extents;
        use crate::game_logic::{KindOf, ThingTemplate};

        let (source_pos, team, plan) = {
            let obj = self.objects.get_mut(&caster_id)?;
            obj.install_spectre_gunship_deployment_if_needed();
            let source_pos = obj.get_position();
            let team = obj.team;
            let dep = obj.spectre_gunship_deployment.as_mut()?;
            let (minx, minz, maxx, maxz) = default_map_extents();
            // Prefer live terrain extents when present.
            let (minx, minz, maxx, maxz) = if let Some(t) = self.terrain.as_ref() {
                // Best-effort residual: keep default if no extent API.
                let _ = t;
                (minx, minz, maxx, maxz)
            } else {
                (minx, minz, maxx, maxz)
            };
            let plan = dep.plan_initiate(source_pos, target_pos, minx, minz, maxx, maxz);
            self.spectre_gunship_deployment_reg.record_initiate();
            (source_pos, team, plan)
        };
        let _ = source_pos;

        // C++ initiateIntent: if prior gunship exists, only `m_gunshipID = INVALID_ID`.
        // `disengageAndDepartAO` is commented out — prior ship keeps orbiting.
        // Clear prior gunship residual = unbind id, never destroy.
        // Wave 750: no mid-frame HP zero, no host_damage_log::record, no
        // gameworld_damage_authority_live() kill path (C++ never kills on recast).
        if plan.replace_prior.is_some() {
            if let Some(dep) = self
                .objects
                .get_mut(&caster_id)
                .and_then(|o| o.spectre_gunship_deployment.as_mut())
            {
                dep.clear_gunship();
            }
            self.spectre_gunship_deployment_reg.record_prior_clear();
        }

        // Ensure gunship template exists.
        if !self.templates.contains_key(&plan.gunship_template) {
            let mut tpl = ThingTemplate::new(&plan.gunship_template);
            tpl.add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .set_health(500.0);
            self.templates.insert(plan.gunship_template.clone(), tpl);
        }

        let caster_owner = self.objects.get(&caster_id).and_then(|o| o.owner_player_id);
        let gunship_id = self.create_object(&plan.gunship_template, team, plan.spawn_pos)?;
        if let Some(g) = self.objects.get_mut(&gunship_id) {
            g.producer_id = Some(caster_id);
            if let Some(owner) = caster_owner {
                g.owner_player_id = Some(owner);
            }
            g.thing.template.add_kind_of(KindOf::Selectable);
            g.set_orientation(plan.orientation);
            // Preferred altitude residual.
            let mut p = g.get_position();
            p.y = plan.spawn_pos.y;
            g.set_position(p);
            // C++ SpectreGunshipUpdate::initiateIntent: INSERTING, doors CLOSING,
            // afterburners on, panic loco, overridable reticle dest.
            g.set_special_power_overridable_destination(
                target_pos,
                Some(crate::command_system::SpecialPowerType::SpectreGunship),
            );
            g.install_spectre_gunship_update_if_needed();
            let mut flight =
                crate::game_logic::host_spectre_gunship_update::HostSpectreGunshipUpdateData::initiate_at(
                    target_pos,
                );
            if let Some(existing) = g.spectre_gunship_update.as_ref() {
                flight.orbit_radius = existing.orbit_radius;
                flight.orbit_frames = existing.orbit_frames;
                flight.orbit_insertion_slope = existing.orbit_insertion_slope;
                flight.attack_area_radius = existing.attack_area_radius;
                flight.targeting_reticle_radius = existing.targeting_reticle_radius;
                flight.preferred_elevation = existing.preferred_elevation;
                flight.initiate(target_pos);
            }
            crate::game_logic::host_spectre_gunship_update::apply_spectre_door_and_afterburner(
                g,
                flight.door_opening,
                flight.afterburners_on,
            );
            crate::game_logic::host_upgrade_module_residuals::apply_choose_locomotor_set(
                g,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Panic,
                false,
            );
            g.spectre_gunship_update = Some(flight);
        }
        if let Some(dep) = self
            .objects
            .get_mut(&caster_id)
            .and_then(|o| o.spectre_gunship_deployment.as_mut())
        {
            dep.bind_gunship(gunship_id);
        }
        self.spectre_gunship_deployment_reg.record_spawn();
        // C++ SpectreGunshipDeploymentUpdate::initiateIntent
        // TheGameLogic->selectObject(newGunship, TRUE, playerMask, TRUE).
        if let Some(pid) = self
            .objects
            .get(&caster_id)
            .and_then(|o| self.player_owner_for_host_object(o))
        {
            self.select_object_list(1u32 << pid.min(31), vec![gunship_id], true);
        }
        // C++ friend_enableAfterburners(TRUE) starts Afterburner per-unit sound.
        if let Some(g) = self.objects.get(&gunship_id) {
            let pos = g.get_position();
            let template_name = g.template_name.clone();
            self.queue_afterburner_per_unit_sound(gunship_id, &template_name, pos, true);
        }
        if let Some(g) = self.objects.get_mut(&gunship_id) {
            g.jet_ai.afterburner_sound_playing = true;
        }
        Some(gunship_id)
    }

    /// C++ SpectreGunshipUpdate::update insertion / orbit / departure residual.
    pub(in super::super) fn update_spectre_gunship_flights(&mut self) {
        use crate::game_logic::host_spectre_gunship_update::apply_spectre_door_and_afterburner;
        use crate::game_logic::host_upgrade_module_residuals::{
            HostLocomotorSetKind, apply_choose_locomotor_set,
        };
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.spectre_gunship_update.is_some())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        let mut audio: Vec<(ObjectId, String, Vec3, bool)> = Vec::new();
        for id in ids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            if o.status.destroyed || !o.is_alive() {
                continue;
            }
            if o.spectre_gunship_update.as_ref().is_none_or(|data| {
                data.status
                    == crate::game_logic::host_spectre_gunship_update::HostGunshipStatus::Idle
            }) {
                continue;
            }
            let pos = o.get_position();
            let facing = o.get_orientation();
            let Some(data) = o.spectre_gunship_update.as_mut() else {
                continue;
            };
            let was_ab = data.afterburners_on;
            let tick = data.tick(pos, facing, frame);
            o.set_position(tick.pos);
            o.movement.velocity = tick.vel;
            if tick.vel.length_squared() > 1.0e-6 {
                o.set_orientation(tick.vel.z.atan2(tick.vel.x));
            }
            apply_spectre_door_and_afterburner(o, tick.door_opening, tick.afterburners_on);
            apply_choose_locomotor_set(
                o,
                if tick.panic_loco {
                    HostLocomotorSetKind::Panic
                } else {
                    HostLocomotorSetKind::Normal
                },
                false,
            );
            if was_ab != tick.afterburners_on {
                audio.push((id, o.template_name.clone(), tick.pos, tick.afterburners_on));
            }
            if tick.destroy {
                destroy.push(id);
            }
        }
        for (id, template_name, pos, on) in audio {
            if let Some(o) = self.objects.get_mut(&id) {
                o.jet_ai.afterburner_sound_playing = on;
            }
            self.queue_afterburner_per_unit_sound(id, &template_name, pos, on);
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
            if let Some(o) = self.objects.get_mut(&id) {
                o.spectre_gunship_update = None;
            }
        }
    }

    /// C++ ObjectCreationList CreateDebris residual with disposition force.
    pub fn spawn_ocl_create_debris(
        &mut self,
        plan: &crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan,
        team: Team,
        origin: Vec3,
        inherit_vel: Vec3,
        owner_player_id: Option<u32>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_ocl_create_debris::{
            debris_initial_velocity, spin_rate_rad_per_frame,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        self.ocl_create_debris_reg.record_plan(plan.disposition);
        let mut out = Vec::new();
        let name = if plan.model_or_template.is_empty() {
            "GenericDebris".to_string()
        } else {
            plan.model_or_template.clone()
        };
        if !self.templates.contains_key(&name) {
            let mut t = ThingTemplate::new(&name);
            t.set_health(plan.mass.max(1.0) * 10.0)
                .add_kind_of(KindOf::Projectile);
            self.templates.insert(name.clone(), t);
        }
        for i in 0..plan.count.max(1) {
            let mut pos = origin + plan.offset;
            // slight index scatter residual
            pos.x += (i as f32) * 0.5;
            pos.z += (i as f32) * 0.35;
            let Some(id) = self.create_object_for_owner_or_team(&name, team, owner_player_id, pos)
            else {
                continue;
            };
            let vel = debris_initial_velocity(
                plan.disposition,
                inherit_vel,
                i,
                plan.min_force,
                plan.max_force,
                plan.min_pitch_deg,
                plan.max_pitch_deg,
            );
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                // spin residual into orientation rate if field exists — use orientation nudge
                let spin = spin_rate_rad_per_frame(plan.spin_rate_deg);
                o.set_orientation(o.get_orientation() + spin * (i as f32 + 1.0));
                o.set_extra_friction(plan.extra_friction);
                // C++ ObjectCreationList.cpp:1121-1123 flying/up/random debris.
                if crate::game_logic::host_ocl_create_debris::disposition_enables_bounce(
                    plan.disposition,
                ) {
                    o.set_extra_bounciness(plan.extra_bounciness);
                    o.set_allow_bouncing(true);
                }
                if !plan.bounce_sound.is_empty() {
                    o.set_bounce_sound(plan.bounce_sound.clone());
                }
            }
            self.ocl_create_debris_reg.record_spawn(plan.disposition);
            out.push(id);
        }
        out
    }

    /// C++ FireWeaponNugget::create residual — spawn projectile template toward target.
    pub fn execute_ocl_fire_weapon(
        &mut self,
        ocl_or_weapon: &str,
        source_id: ObjectId,
        primary: Vec3,
        secondary: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_ocl_fire_weapon_attack::fire_weapon_plan_for_ocl;
        use crate::game_logic::{KindOf, ThingTemplate};

        let plan = fire_weapon_plan_for_ocl(ocl_or_weapon)?;
        self.ocl_fire_weapon_attack_reg
            .record_fire_weapon(&plan.weapon_name);
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        if !self.templates.contains_key(&plan.projectile_template) {
            let mut t = ThingTemplate::new(&plan.projectile_template);
            t.set_health(100.0)
                .add_kind_of(KindOf::Projectile)
                .add_kind_of(KindOf::Aircraft);
            self.templates.insert(plan.projectile_template.clone(), t);
        }
        // Spawn at primary (launcher) residual; smart-bomb course toward secondary.
        let id = self.create_object(&plan.projectile_template, team, primary)?;
        if let Some(o) = self.objects.get_mut(&id) {
            o.note_producer(source_id);
            let yaw = (secondary.z - primary.z).atan2(secondary.x - primary.x);
            o.set_orientation(yaw);
            // C++ NeutronMissileUpdate loft residual (preferred over flat smart-bomb).
            o.ensure_neutron_missile_update(secondary, Some(source_id), self.frame);
            if o.neutron_missile_update.is_some() {
                self.neutron_missile_update_reg.record_launch();
                // C++ NeutronMissileUpdate DeliveryDecal residual on launcher/missile.
                let _ = o.create_delivery_radius_decal(secondary, self.frame);
            } else {
                // Non-neutron projectiles: smart-bomb course residual.
                let mut p = o.get_position();
                p.y = p.y.max(80.0);
                o.set_position(p);
                let _ = o.set_smart_bomb_target(secondary);
                let dx = secondary.x - primary.x;
                let dz = secondary.z - primary.z;
                let dist = (dx * dx + dz * dz).sqrt().max(1.0);
                o.movement.velocity = glam::Vec3::new(dx / dist * 25.0, 5.0, dz / dist * 25.0);
            }
        }
        self.ocl_fire_weapon_attack_reg.record_projectile();
        // DeliveryDecal also on launcher residual (RadiusDecalUpdate on SW building).
        if self
            .objects
            .get(&id)
            .and_then(|o| o.neutron_missile_update.as_ref())
            .is_some()
        {
            let _ = self.create_delivery_radius_decal(source_id, secondary);
        }
        Some(id)
    }

    /// C++ FireWeaponPower::doSpecialPowerAtLocation residual.
    ///
    /// Leftover `FireWeaponPower` already matches C++ (reloadAllAmmo +
    /// aiAttackPosition + turret aim). Live host objects use the same
    /// residual: reload, queue maxShots attack-position, aim turrets.
    pub fn activate_fire_weapon_power(&mut self, source_id: ObjectId, location: Vec3) -> bool {
        // Leftover FireWeaponPower when dual-world object is registered.
        if let Some(obj) = gamelogic::helpers::TheGameLogic::find_object_by_id(source_id.0) {
            if let Ok(guard) = obj.read() {
                let loc = gamelogic::common::Coord3D::new(location.x, location.z, location.y);
                guard.do_special_power_at_location(
                    "SpecialPowerBattleshipBombardment",
                    &loc,
                    gamelogic::object_creation_list::nuggets::INVALID_ANGLE,
                    gamelogic::object::special_power_module::SpecialPowerCommandOptions::NONE,
                    false,
                );
            }
        }

        let ok = self
            .objects
            .get_mut(&source_id)
            .is_some_and(|o| o.activate_fire_weapon_power(Some((location.x, location.z))));
        if !ok {
            return false;
        }
        // C++ aiAttackPosition(loc, maxShotsToFire, CMD_FROM_AI)
        if let Some(o) = self.objects.get_mut(&source_id) {
            o.target_location = Some(location);
            o.set_ai_state(crate::game_logic::AIState::Attacking);
            let shots = o
                .fire_weapon_power
                .as_ref()
                .map(|r| r.shots_remaining as i32)
                .unwrap_or(1);
            o.set_max_shots_to_fire(shots);
        }
        // C++ for i in MAX_TURRETS: setTurretTargetPosition(i, loc)
        self.set_turret_target_position(source_id, Some(location));
        true
    }

    /// C++ FireWeaponPower::doSpecialPowerAtObject residual.
    /// Reload, `aiAttackObject`, and `setTurretTargetObject` for every turret.
    pub fn activate_fire_weapon_power_at_object(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        if self.objects.get(&target_id).is_none() {
            return false;
        }
        if let Some(obj) = gamelogic::helpers::TheGameLogic::find_object_by_id(source_id.0) {
            if let Ok(guard) = obj.read() {
                guard.do_special_power_at_object(
                    "SpecialPowerBattleshipBombardment",
                    target_id.0,
                    gamelogic::object::special_power_module::SpecialPowerCommandOptions::NONE,
                    false,
                );
            }
        }

        let ok = self
            .objects
            .get_mut(&source_id)
            .is_some_and(|o| o.activate_fire_weapon_power_at_object(target_id));
        if !ok {
            return false;
        }
        if let Some(o) = self.objects.get_mut(&source_id) {
            o.set_target(Some(target_id));
            o.target_location = None;
            o.set_ai_state(crate::game_logic::AIState::Attacking);
            let shots = o
                .fire_weapon_power
                .as_ref()
                .map(|r| r.shots_remaining as i32)
                .unwrap_or(1);
            o.set_max_shots_to_fire(shots);
        }
        self.set_turret_target_object(source_id, Some(target_id), false);
        true
    }

    /// C++ AttackNugget::create residual — multi-shot attack position + delivery decal.
    pub fn execute_ocl_attack(
        &mut self,
        ocl_name: &str,
        source_id: ObjectId,
        target: Vec3,
    ) -> bool {
        use crate::game_logic::host_ocl_fire_weapon_attack::attack_plan_for_ocl;

        let plan = match attack_plan_for_ocl(ocl_name) {
            Some(p) => p,
            None => return false,
        };
        self.ocl_fire_weapon_attack_reg
            .record_attack(plan.number_of_shots);
        // FireWeaponPower-style multi-shot attack residual on source.
        if let Some(o) = self.objects.get_mut(&source_id) {
            let _ = o.activate_fire_weapon_power(Some((target.x, target.z)));
            if let Some(req) = o.fire_weapon_power.as_mut() {
                req.shots_remaining = plan.number_of_shots.max(1);
                req.target_x = target.x;
                req.target_z = target.z;
                req.has_location = true;
            }
            // Attack target location residual.
            o.target_location = Some(target);
        }
        // C++ RadiusDecalUpdate delivery decal residual.
        if plan.delivery_decal_radius > 0.0 {
            let _ = self.create_delivery_radius_decal(source_id, target);
            self.ocl_fire_weapon_attack_reg.record_decal();
        }
        // C++ ScudStorm ClipSize-9 ScudStormMissile ballistic spawns (scatter table).
        if plan.ocl_name.contains("ScudStorm") {
            self.spawn_scud_storm_missile_flight(source_id, target);
        }
        true
    }

    /// Schedule ClipSize-staggered ScudStormMissile ballistic spawns (scatter table).
    pub fn spawn_scud_storm_missile_flight(&mut self, source_id: ObjectId, target: Vec3) -> u32 {
        use crate::game_logic::special_power_strikes::scud_storm_points;

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let launch = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let points = scud_storm_points(target);
        let before = self.scud_storm_missile_flight_reg.scheduled;
        let _ = team;
        self.scud_storm_missile_flight_reg.schedule_wave(
            self.frame,
            source_id.0,
            0,
            launch,
            &points,
        );
        self.scud_storm_missile_flight_reg
            .scheduled
            .saturating_sub(before)
    }

    pub(in super::super) fn ensure_scud_storm_missile_template(&mut self) {
        use crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_OBJECT;
        use crate::game_logic::{KindOf, ThingTemplate};
        if !self.templates.contains_key(SCUD_STORM_MISSILE_OBJECT) {
            let mut t = ThingTemplate::new(SCUD_STORM_MISSILE_OBJECT);
            t.set_health(10000.0)
                .add_kind_of(KindOf::Projectile)
                .add_kind_of(KindOf::Aircraft);
            self.templates
                .insert(SCUD_STORM_MISSILE_OBJECT.to_string(), t);
        }
    }

    pub(in super::super) fn spawn_due_scud_storm_missiles(&mut self) {
        use crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightData;
        use crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_OBJECT;

        // C++ WeaponSet.cpp:428-432 — dead building cannot fire remaining clip shots.
        // Leftover weapon_set_able already matches; live registry must drop unlaunched.
        self.cancel_unlaunched_scud_storm_for_dead_pads();

        let due = self
            .scud_storm_missile_flight_reg
            .take_due_spawns(self.frame);
        if due.is_empty() {
            return;
        }
        self.ensure_scud_storm_missile_template();
        let mut n = 0u32;
        for p in due {
            let team = self
                .objects
                .get(&ObjectId(p.source_id))
                .map(|o| o.team)
                .unwrap_or(Team::Neutral);
            let Some(mid) = self.create_object(SCUD_STORM_MISSILE_OBJECT, team, p.launch) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&mid) {
                o.producer_id = Some(ObjectId(p.source_id));
                // C++ Weapon.cpp:1109-1112 / ObjectCreationList.cpp:386-393 setCreator.
                o.bind_special_power_completion_creator(p.source_id);
                o.scud_storm_missile_flight = Some(HostScudStormMissileFlightData::start(
                    p.launch,
                    p.target,
                    p.missile_index,
                    Some(p.source_id),
                ));
            }

            n = n.saturating_add(1);
        }
        self.scud_storm_missile_flight_reg.record_launch(n);
    }

    /// C++ AttackNugget fires via the pad weapon; dead pad stops the remaining salvo.
    fn cancel_unlaunched_scud_storm_for_dead_pads(&mut self) {
        let mut dead = Vec::new();
        for p in &self.scud_storm_missile_flight_reg.pending {
            if !dead.contains(&p.source_id) && !self.scud_storm_pad_can_fire(p.source_id) {
                dead.push(p.source_id);
            }
        }
        for id in dead {
            self.scud_storm_missile_flight_reg
                .cancel_unlaunched_for_source(id);
        }
    }

    /// Leftover `isEffectivelyDead` (C++ WeaponSet.cpp:428-432) plus host `is_alive`.
    fn scud_storm_pad_can_fire(&self, source_id: u32) -> bool {
        if let Some(obj) = gamelogic::helpers::TheGameLogic::find_object_by_id(source_id) {
            if let Ok(guard) = obj.read() {
                if guard.is_effectively_dead() || guard.is_destroyed() {
                    return false;
                }
            }
        }
        self.objects
            .get(&ObjectId(source_id))
            .is_some_and(|o| o.is_alive())
    }

    /// C++ ObjectCreationList::create residual after OCLSpecialPower plan.
    ///
    /// Spawns transport at creation_coord (DeliverPayload) or CreateObject names
    /// at target (SpyDrone). Payload object is tagged at target for drop residual
    /// unless TransportOnly mode (Leaflet/Paradrop host owns impact).
    pub fn execute_ocl_special_power(
        &mut self,
        power_template: &str,
        caster_id: ObjectId,
        target_pos: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_ocl_special_power::{
            OclExecuteMode, create_object_for_ocl, deliver_payload_for_ocl,
            ocl_execute_mode_for_template,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let plan = self.plan_ocl_special_power(power_template, caster_id, target_pos)?;
        // C++ createOwner=false (USE_OWNER_OBJECT / doSpecialPower): reuse firer.
        if !plan.create_owner {
            self.ocl_special_power_reg.record_transport_spawn();
            return Some(caster_id);
        }
        let (team, source_owner_player_id) = {
            let caster = self.objects.get(&caster_id)?;
            let owner_player_id = if caster.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(caster)?)
            } else {
                None
            };
            (caster.team, owner_player_id)
        };
        let mode = ocl_execute_mode_for_template(power_template);

        if create_object_for_ocl(&plan.ocl_name).is_some()
            || matches!(mode, OclExecuteMode::CreateObject)
        {
            let create = create_object_for_ocl(&plan.ocl_name)?;
            let mut last = None;
            for name in &create.object_names {
                for _ in 0..create.count.max(1) {
                    if !self.templates.contains_key(name) {
                        let mut tpl = ThingTemplate::new(name);
                        tpl.add_kind_of(KindOf::Vehicle)
                            .add_kind_of(KindOf::Selectable)
                            .set_health(100.0);
                        self.templates.insert(name.clone(), tpl);
                    }
                    if let Some(id) = self.create_object_for_owner_or_team(
                        name,
                        team,
                        source_owner_player_id,
                        plan.target_coord,
                    ) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.producer_id = Some(caster_id);
                            // C++ ObjectCreationList.cpp:386-393 setCreator on spawned object.
                            o.bind_special_power_completion_creator(caster_id.0);
                        }

                        self.ocl_special_power_reg.record_create_object_spawn();
                        last = Some(id);
                    }
                }
            }
            return last;
        }

        let deliver = deliver_payload_for_ocl(&plan.ocl_name)?;
        if !self.templates.contains_key(&deliver.transport) {
            let mut tpl = ThingTemplate::new(&deliver.transport);
            tpl.add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .set_health(500.0);
            self.templates.insert(deliver.transport.clone(), tpl);
        }
        let transport_id = self.create_object_for_owner_or_team(
            &deliver.transport,
            team,
            source_owner_player_id,
            plan.creation_coord,
        )?;
        if let Some(t) = self.objects.get_mut(&transport_id) {
            t.producer_id = Some(caster_id);
            // C++ ObjectCreationList.cpp:386-393 setCreator on DeliverPayload transport.
            t.bind_special_power_completion_creator(caster_id.0);
            let p = t.get_position();
            let yaw = (plan.target_coord.z - p.z).atan2(plan.target_coord.x - p.x);
            t.set_orientation(yaw);
            if deliver.start_at_preferred_height {
                let mut pos = p;
                pos.y = pos.y.max(120.0);
                t.set_position(pos);
            }
        }

        self.ocl_special_power_reg.record_transport_spawn();

        if mode == OclExecuteMode::TransportOnly {
            return Some(transport_id);
        }

        // C++ DeliverPayload: payload spawns after approach/door delay, not at fire.
        // Queue SuperweaponOclBomb mission; update_deliver_payloads drops payload.
        if !self.templates.contains_key(&deliver.payload) {
            let mut tpl = ThingTemplate::new(&deliver.payload);
            let pl = deliver.payload.to_ascii_lowercase();
            if pl.contains("bomb") || pl.contains("moab") || pl.contains("missile") {
                tpl.add_kind_of(KindOf::Projectile);
            } else if pl.contains("infantry") || pl.contains("ranger") {
                tpl.add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Selectable);
            } else {
                tpl.add_kind_of(KindOf::Vehicle);
            }
            tpl.set_health(100.0);
            self.templates.insert(deliver.payload.clone(), tpl);
        }
        // Align DeliverPayload drop with host superweapon impact_delay residual.
        let impact_delay = {
            use crate::game_logic::special_power_strikes::{
                A10_STRIKE_IMPACT_DELAY_FRAMES, DAISY_CUTTER_IMPACT_DELAY_FRAMES,
            };
            let n = power_template.to_ascii_lowercase();
            if n.contains("a10") {
                A10_STRIKE_IMPACT_DELAY_FRAMES
            } else if n.contains("daisy") || n.contains("moab") || n.contains("fuelair") {
                DAISY_CUTTER_IMPACT_DELAY_FRAMES
            } else {
                DAISY_CUTTER_IMPACT_DELAY_FRAMES
            }
        };
        let mission_id = self
            .host_deliver_payloads
            .queue_superweapon_ocl_bomb_for_owner(
                caster_id,
                team,
                source_owner_player_id,
                plan.target_coord,
                self.frame,
                deliver.payload.clone(),
                impact_delay,
            );
        // Bind live transport object to cargo flight residual for approach motion.
        if let Some(m) = self.host_deliver_payloads.get_mut(mission_id) {
            m.transport_object_id = Some(transport_id);
            m.transport_template = deliver.transport.clone();
        }
        if let Some(flight) = self.host_deliver_payloads.cargo_flight_mut(mission_id) {
            flight.transport_template = deliver.transport.clone();
            // Seat CreateAtEdge on leftover/live map rim, not residual 0..500.
            let mut edge = plan.creation_coord;
            if deliver.start_at_preferred_height {
                edge.y = edge.y.max(flight.preferred_height);
            }
            flight.edge_spawn_pos = edge;
            flight.current_pos = edge;
            flight.reapproach_pos = edge;
            let dx = plan.target_coord.x - edge.x;
            let dz = plan.target_coord.z - edge.z;
            let dlen = (dx * dx + dz * dz).sqrt().max(0.001);
            flight.dir_x = dx / dlen;
            flight.dir_z = dz / dlen;
            flight.previous_distance =
                crate::game_logic::host_deliver_payload::horizontal_distance_xz(
                    edge,
                    plan.target_coord,
                );
            if let Some(t) = self.objects.get_mut(&transport_id) {
                t.set_position(edge);
                let yaw = flight.dir_z.atan2(flight.dir_x);
                t.set_orientation(yaw);
            }
        }
        Some(transport_id)
    }

    /// C++ `OCLSpecialPower::doSpecialPower` — no-target fire at owner pos
    /// with `createOwner=false` so DeliverPayload reuses the firing object.
    pub fn execute_ocl_special_power_no_target(
        &mut self,
        power_template: &str,
        caster_id: ObjectId,
    ) -> Option<ObjectId> {
        let pos = self.objects.get(&caster_id)?.get_position();
        let mut plan = self.plan_ocl_special_power(power_template, caster_id, pos)?;
        plan.create_owner =
            crate::game_logic::host_ocl_special_power::ocl_create_owner_for_no_target();
        plan.creation_coord = pos;
        plan.target_coord = pos;
        if !plan.create_owner {
            self.ocl_special_power_reg.record_transport_spawn();
            return Some(caster_id);
        }
        self.execute_ocl_special_power(power_template, caster_id, pos)
    }

    /// C++ `TheTerrainLogic->getExtent()` for OCL CreateAtEdge.
    /// Prefer leftover TerrainLogic active boundary; else live world_min/world_max.
    fn ocl_map_extents(&self) -> (f32, f32, f32, f32) {
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().try_read() {
            let ext = terrain.get_extent();
            if ext.hi.x > ext.lo.x && ext.hi.y > ext.lo.y {
                return (ext.lo.x, ext.lo.y, ext.hi.x, ext.hi.y);
            }
        }
        (
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        )
    }

    pub fn plan_ocl_special_power(
        &mut self,
        power_template: &str,
        caster_id: ObjectId,
        target_pos: Vec3,
    ) -> Option<crate::game_logic::host_ocl_special_power::OclSpecialPowerSpawnPlan> {
        use crate::game_logic::host_ocl_special_power::{
            find_ocl_name, peel_for_special_power, plan_ocl_special_power_at_location,
        };
        let (source_pos, unlocked) = {
            let caster = self.objects.get(&caster_id)?;
            let source_pos = caster.get_position();
            // C++ findOCL uses getControllingPlayer(), not a faction-wide slot.
            let unlocked: Vec<String> = self
                .player_owner_for_host_object(caster)
                .and_then(|pid| self.get_player(pid))
                .map(|p| p.unlocked_sciences.iter().cloned().collect())
                .unwrap_or_default();
            (source_pos, unlocked)
        };
        let used_upgrade = peel_for_special_power(power_template)
            .map(|p| {
                let resolved =
                    find_ocl_name(p, |s| unlocked.iter().any(|u| u.eq_ignore_ascii_case(s)));
                resolved != p.default_ocl
            })
            .unwrap_or(false);
        let (minx, minz, maxx, maxz) = self.ocl_map_extents();
        let plan = {
            use gamelogic::ai::pathfind_astar::PathfindCellType;
            let grid = &self.pathfinding_system.grid;
            let is_clear = |pos: Vec3| {
                let cell = grid.world_to_grid(pos);
                matches!(grid.cell_type(cell), PathfindCellType::Clear) && !grid.is_blocked(cell)
            };
            plan_ocl_special_power_at_location(
                power_template,
                source_pos,
                target_pos,
                |s| unlocked.iter().any(|u| u.eq_ignore_ascii_case(s)),
                minx,
                minz,
                maxx,
                maxz,
                Some(&is_clear as &dyn Fn(Vec3) -> bool),
            )
        }?;

        self.ocl_special_power_reg.record_plan(&plan, used_upgrade);
        Some(plan)
    }

    pub fn set_smart_bomb_target(&mut self, bomb_id: ObjectId, target: Vec3) -> bool {
        let Some(obj) = self.objects.get_mut(&bomb_id) else {
            return false;
        };
        if obj.set_smart_bomb_target(target) {
            self.smart_bomb_target_homing_reg.record_target();
            true
        } else {
            false
        }
    }

    /// C++ SmartBombTargetHomingUpdate::update residual.
    /// C++ SlowDeathBehavior on FuelAir gas: midpoint flame + final detonation.
    pub fn update_fuel_air_gas_slow_death(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_fuel_air_gas_slow_death::FuelAirGasTickEvent;

        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.fuel_air_gas_slow_death.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();

        let mut destroy: Vec<ObjectId> = Vec::new();
        for id in ids {
            let pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(Vec3::ZERO);
            let producer = self.objects.get(&id).and_then(|o| o.producer_id);
            let team = self
                .objects
                .get(&id)
                .map(|o| o.team)
                .unwrap_or(Team::Neutral);
            let ev = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let Some(data) = o.fuel_air_gas_slow_death.as_mut() else {
                    continue;
                };
                data.tick(self.frame)
            };
            match ev {
                FuelAirGasTickEvent::None => {}
                FuelAirGasTickEvent::InitialFx => {
                    let _ = self.combat_particles.spawn(
                        CombatParticleKind::DeathExplosion,
                        pos,
                        self.frame,
                        Some(id),
                        None,
                    );
                }
                FuelAirGasTickEvent::MidpointFlame {
                    damage,
                    radius,
                    weapon: _,
                } => {
                    self.fuel_air_gas_reg.record_midpoint();
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        pos,
                        damage,
                        radius,
                        DamageType::Flame,
                    );
                }
                FuelAirGasTickEvent::FinalDetonation {
                    damage,
                    radius,
                    weapon: _,
                    fx: _,
                } => {
                    self.fuel_air_gas_reg.record_final();
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        pos,
                        damage,
                        radius,
                        DamageType::Explosive,
                    );
                    let _ = self.combat_particles.spawn(
                        CombatParticleKind::DeathExplosion,
                        pos,
                        self.frame,
                        Some(id),
                        None,
                    );
                    destroy.push(id);
                }
            }
        }
        for id in destroy {
            self.fuel_air_gas_reg.record_destroy();
            self.mark_object_for_destruction(id, None);
        }
    }

    pub(crate) fn apply_fuel_air_radius_damage(
        &mut self,
        source_id: ObjectId,
        producer: Option<ObjectId>,
        _source_team: Team,
        epicenter: Vec3,
        damage: f32,
        radius: f32,
        damage_type: crate::game_logic::combat::DamageType,
    ) {
        let r2 = radius * radius;
        let killer = producer.or(Some(source_id));
        let victims: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(oid, o)| {
                **oid != source_id && o.is_alive() && {
                    let p = o.get_position();
                    let dx = p.x - epicenter.x;
                    let dz = p.z - epicenter.z;
                    dx * dx + dz * dz <= r2
                }
            })
            .map(|(id, _)| *id)
            .collect();
        let mut destroy_ids = Vec::new();
        for vid in victims {
            if let Some(t) = self.objects.get_mut(&vid) {
                if t.take_damage_from_typed(damage, killer, damage_type) {
                    destroy_ids.push(vid);
                }
            }
        }
        for vid in destroy_ids {
            self.mark_object_for_destruction(vid, None);
        }
    }

    pub(in super::super) fn update_smart_bomb_target_homing(&mut self) {
        use crate::game_logic::host_smart_bomb_target_homing::SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.smart_bomb_target_homing.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let (pos, hat) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                let pos = obj.get_position();
                let terrain_y = self.terrain_height_at(pos).unwrap_or(0.0);
                (pos, pos.y - terrain_y)
            };
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(h) = obj.smart_bomb_target_homing.as_ref() else {
                continue;
            };
            if let Some(np) = h.tick(pos, hat.max(0.0)) {
                // Only apply when clearly above threshold (tick already gates).
                if hat >= SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN {
                    obj.set_position(np);
                    self.smart_bomb_target_homing_reg.record_steer();
                }
            }
        }
    }

    pub(in super::super) fn update_checkpoint_update(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.checkpoint_update.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let (team, pos, vision, needs_scan) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                let Some(cp) = obj.checkpoint_update.as_ref() else {
                    continue;
                };
                (
                    obj.team,
                    obj.get_position(),
                    cp.vision_range.max(obj.vision_range),
                    cp.needs_scan(),
                )
            };

            let scan = if needs_scan {
                self.checkpoint_update_reg.record_scan();
                let mut enemy = false;
                let mut ally = false;
                for (oid, o) in self.objects.iter() {
                    if *oid == id || !o.is_alive() {
                        continue;
                    }
                    let op = o.get_position();
                    let dx = op.x - pos.x;
                    let dz = op.z - pos.z;
                    if (dx * dx + dz * dz).sqrt() > vision {
                        continue;
                    }
                    if o.is_targetable_by_enemy_of(team) {
                        enemy = true;
                    } else if o.team == team {
                        ally = true;
                    }
                    if enemy && ally {
                        break;
                    }
                }
                Some((enemy, ally))
            } else {
                None
            };

            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(cp) = obj.checkpoint_update.as_mut() {
                    let was_open = cp.open;
                    let changed = cp.tick(scan);
                    if changed {
                        if cp.open && !was_open {
                            self.checkpoint_update_reg.record_open();
                        } else if !cp.open && was_open {
                            self.checkpoint_update_reg.record_close();
                        }
                        // Door model condition residual.
                        if let Some(name) = cp.door_anim.model_condition() {
                            if let Some(bit) =
                                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                                    name,
                                )
                            {
                                // Clear both door bits then set active.
                                if let Some(b) =
                                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                                        "DOOR_1_OPENING",
                                    )
                                {
                                    obj.model_condition_bits &= !(1u128 << b);
                                }
                                if let Some(b) =
                                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                                        "DOOR_1_CLOSING",
                                    )
                                {
                                    obj.model_condition_bits &= !(1u128 << b);
                                }
                                obj.model_condition_bits |= 1u128 << bit;
                            }
                        }
                    }
                }
            }
        }
    }

    pub(in super::super) fn update_radius_decal_update(&mut self) {
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.radius_decal_update.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let attacking = obj.status.attacking
                || matches!(obj.ai_state, crate::game_logic::AIState::Attacking);
            let Some(rd) = obj.radius_decal_update.as_mut() else {
                continue;
            };
            if !rd.awake {
                continue;
            }
            self.radius_decal_update_reg.record_update();
            if rd.tick(frame, attacking) {
                self.radius_decal_update_reg.record_kill(true);
            }
        }
    }

    pub(in super::super) fn update_prone_update(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.prone_update.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(pu) = obj.prone_update.as_mut() else {
                continue;
            };
            let was_prone = pu.is_prone();
            let recovered = pu.tick();
            let prone_now = pu.is_prone();
            let no_attack = pu.no_attack;
            let model_prone = pu.model_prone;
            if recovered {
                self.prone_update_reg.record_recovery();
            }
            // Mirror NO_ATTACK status bit residual.
            if no_attack {
                let _ = obj.apply_status_bits_upgrade_masks(&["NO_ATTACK"], &[]);
            } else if was_prone && !prone_now {
                let _ = obj.apply_status_bits_upgrade_masks(&[], &["NO_ATTACK"]);
            }
            if model_prone {
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    obj.model_condition_bits |= 1u128 << bit;
                }
            } else if recovered {
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
            }
        }
    }

    pub(in super::super) fn update_animation_steering(&mut self) {
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.animation_steering.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let turning = obj.physics_turning;
            let Some(anim) = obj.animation_steering.as_mut() else {
                continue;
            };
            if let Some(cond) = anim.tick(frame, turning) {
                self.animation_steering_reg.record_transition(cond);
            }
        }
    }

    pub(in super::super) fn update_enemy_near(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.enemy_near.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let (team, pos, vision, needs_scan) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                let Some(en) = obj.enemy_near.as_ref() else {
                    continue;
                };
                (
                    obj.team,
                    obj.get_position(),
                    en.vision_range.max(obj.vision_range),
                    en.scan_delay == 0,
                )
            };

            let enemy_present = if needs_scan {
                self.enemy_near_reg.record_scan();
                let mut found = false;
                for (oid, o) in self.objects.iter() {
                    if *oid == id || !o.is_alive() {
                        continue;
                    }
                    if !o.is_targetable_by_enemy_of(team) {
                        continue;
                    }
                    let op = o.get_position();
                    let dx = op.x - pos.x;
                    let dz = op.z - pos.z;
                    if (dx * dx + dz * dz).sqrt() <= vision {
                        found = true;
                        break;
                    }
                }
                Some(found)
            } else {
                None
            };

            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(en) = obj.enemy_near.as_mut() {
                    let (near, clear) = en.tick(enemy_present);
                    if near {
                        self.enemy_near_reg.record_near();
                    }
                    if clear {
                        self.enemy_near_reg.record_clear();
                    }
                }
            }
        }
    }

    pub(in super::super) fn update_base_regenerate(&mut self) {
        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.base_regenerate.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_h = obj.health.maximum.max(obj.max_health).max(1.0);
            let cur = obj.health.current;
            let under = obj.status.under_construction;
            let sold = obj.status.sold;
            let amount = {
                let Some(br) = obj.base_regenerate.as_mut() else {
                    continue;
                };
                let had_pending = br.pending_damage;
                let amt = br.tick_heal_amount(frame, cur, max_h, under, sold);
                if had_pending && !br.pending_damage {
                    self.base_regenerate_reg.record_damage_delay();
                }
                amt
            };
            if amount > 0.0 {
                let new_h = (cur + amount).min(max_h);
                obj.health.current = new_h;
                self.base_regenerate_reg.record_heal(amount);
            }
        }
    }

    /// Notify BaseRegenerateUpdate residual after non-heal damage.
    pub fn notify_base_regenerate_damage(&mut self, id: ObjectId, is_healing: bool) {
        let frame = self.frame as u32;
        if let Some(obj) = self.objects.get_mut(&id) {
            if obj.base_regenerate.is_some() {
                obj.notify_base_regenerate_damage(frame, is_healing);
                if !is_healing {
                    self.base_regenerate_reg.record_damage_delay();
                }
            }
        }
    }

    pub(crate) fn apply_fire_spread_tick_event(
        &mut self,
        ev: crate::game_logic::host_fire_spread_log::FireSpreadTickEvent,
    ) {
        use crate::game_logic::host_fire_spread::{HostFireSpreadData, HostFlammableState};
        if let Some(obj) = self.objects.get_mut(&ev.id) {
            let mut fs = obj
                .fire_spread
                .clone()
                .unwrap_or_else(HostFireSpreadData::tree_default);
            fs.state = match ev.state {
                1 => HostFlammableState::Aflame,
                2 => HostFlammableState::Burned,
                _ => HostFlammableState::Normal,
            };
            fs.aflame_end_frame = ev.aflame_end_frame;
            fs.burned_end_frame = ev.burned_end_frame;
            fs.next_spread_frame = ev.next_spread_frame;
            fs.flame_damage_accum = ev.flame_damage_accum;
            fs.spread_try_range = ev.spread_try_range;
            fs.active = true;
            obj.fire_spread = Some(fs);
            if ev.became_burned {
                self.fire_spread_reg.record_burned();
                obj.apply_flammable_extinguish_visuals(true);
            } else if ev.aflame {
                let smolder = obj
                    .fire_spread
                    .as_ref()
                    .map(|f| f.smoldering)
                    .unwrap_or(false);
                obj.apply_flammable_visuals(true, smolder, false);
            }
        }
        if ev.spawn_embers {
            self.fire_spread_reg.record_embers();
            let ocl = self
                .objects
                .get(&ev.id)
                .and_then(|o| o.fire_spread.as_ref().map(|f| f.ocl_embers.clone()))
                .unwrap_or_default();
            let _ = self.spawn_fire_spread_embers(ev.id, ev.pos, &ocl);
        }
        if ev.try_spread {
            self.fire_spread_reg.record_spread();
        }
        if let Some(tid) = ev.ignite_target {
            if self.ignite_object_fire_spread(tid) {
                // counted inside helper
            }
        }
    }

    pub(in super::super) fn update_fire_spread(&mut self) {
        use crate::game_logic::host_fire_spread::fire_spread_center_3d_distance;

        let frame = self.frame as u32;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.has_fire_spread())
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            return;
        }

        let positions: Vec<(ObjectId, Vec3, bool)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.has_fire_spread())
            .map(|(id, o)| {
                let would = o
                    .fire_spread
                    .as_ref()
                    .map(|f| f.would_ignite())
                    .unwrap_or(false);
                (*id, o.get_position(), would)
            })
            .collect();

        let mut ignite_targets: Vec<ObjectId> = Vec::new();
        let mut status_aflame: Vec<ObjectId> = Vec::new();
        let mut status_smolder: Vec<ObjectId> = Vec::new();
        let mut status_burned: Vec<ObjectId> = Vec::new();
        let mut status_normal: Vec<ObjectId> = Vec::new();
        let mut aflame_dots: Vec<(ObjectId, f32)> = Vec::new();
        let mut ember_spawns: Vec<(ObjectId, Vec3, String)> = Vec::new();
        let mut start_sounds: Vec<(ObjectId, Vec3, String)> = Vec::new();
        let mut stop_sounds: Vec<(ObjectId, Vec3, String)> = Vec::new();
        let mut auto_aflame: Vec<(ObjectId, Vec3)> = Vec::new();

        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(fs) = obj.fire_spread.as_mut() else {
                continue;
            };
            let fr = fs.tick_flammable(frame);
            if fr.stop_burning_sound {
                stop_sounds.push((id, pos, fs.burning_sound_name.clone()));
            }
            if fr.became_smoldering {
                status_smolder.push(id);
            }
            if fr.became_burned {
                self.fire_spread_reg.record_burned();
                status_burned.push(id);
            } else if fr.returned_to_normal {
                status_normal.push(id);
            } else if fr.aflame {
                status_aflame.push(id);
                if fr.start_burning_sound {
                    start_sounds.push((id, pos, fs.burning_sound_name.clone()));
                    auto_aflame.push((id, pos));
                }
            }
            if fr.aflame_damage > 0.0 {
                aflame_dots.push((id, fr.aflame_damage));
            }
            let sr = fs.tick_spread(frame);
            let range = fs.spread_try_range;
            if sr.spawn_embers {
                self.fire_spread_reg.record_embers();
                ember_spawns.push((id, pos, fs.ocl_embers.clone()));
            }
            if sr.try_spread {
                self.fire_spread_reg.record_spread();
                let mut best: Option<(ObjectId, f32)> = None;
                for &(oid, op, would) in &positions {
                    if oid == id || !would {
                        continue;
                    }
                    let dist = fire_spread_center_3d_distance(pos, op);
                    if dist <= range && best.map(|(_, d)| dist < d).unwrap_or(true) {
                        best = Some((oid, dist));
                    }
                }
                if let Some((tid, _)) = best {
                    ignite_targets.push(tid);
                }
            }
        }

        for id in status_aflame {
            if let Some(o) = self.objects.get_mut(&id) {
                let smolder = o
                    .fire_spread
                    .as_ref()
                    .map(|f| f.smoldering)
                    .unwrap_or(false);
                o.apply_flammable_visuals(true, smolder, false);
            }
        }
        for id in status_smolder {
            if let Some(o) = self.objects.get_mut(&id) {
                o.apply_flammable_smoldering_visuals();
            }
        }
        for id in status_burned {
            if let Some(o) = self.objects.get_mut(&id) {
                o.apply_flammable_extinguish_visuals(true);
            }
        }
        for id in status_normal {
            if let Some(o) = self.objects.get_mut(&id) {
                o.apply_flammable_extinguish_visuals(false);
            }
        }
        for (id, pos, ocl) in ember_spawns {
            let _ = self.spawn_fire_spread_embers(id, pos, &ocl);
        }
        for (id, pos, sound) in start_sounds {
            self.start_fire_spread_burning_sound(id, pos, &sound);
        }
        for (id, pos, sound) in stop_sounds {
            self.stop_fire_spread_burning_sound(id, pos, &sound);
        }
        for (id, pos) in auto_aflame {
            self.spawn_auto_aflame_particles(id, pos);
        }
        for tid in ignite_targets {
            let _ = self.ignite_object_fire_spread(tid);
        }
        // C++ FlammableUpdate.cpp:202-212 doAflameDamage DAMAGE_FLAME / DEATH_BURNED.
        for (id, dmg) in aflame_dots {
            if let Some(o) = self.objects.get_mut(&id) {
                let _ = o.take_damage_from_typed_death(
                    dmg,
                    Some(id),
                    crate::game_logic::combat::DamageType::Flame,
                    crate::game_logic::host_usa_pilot::HostDeathType::Burned,
                );
            }
        }
    }

    pub(in super::super) fn update_tensile_formations(&mut self) {
        use crate::game_logic::AudioEventRequest;
        use crate::game_logic::host_tensile_formation::{
            TENSILE_BODY_DAMAGED_HEALTH_FRAC, TENSILE_CRACK_SOUND, TENSILE_PROPAGATE_RADIUS,
        };

        let frame = self.frame as u32;
        let members: Vec<(ObjectId, Vec3)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.has_tensile_formation())
            .map(|(id, o)| (*id, o.get_position()))
            .collect();
        if members.is_empty() {
            return;
        }

        let ids: Vec<ObjectId> = members.iter().map(|(id, _)| *id).collect();
        let mut crack_events: Vec<Vec3> = Vec::new();
        let mut propagate_centers: Vec<Vec3> = Vec::new();
        let mut rubble_ids: Vec<ObjectId> = Vec::new();
        let mut position_updates: Vec<(ObjectId, Vec3)> = Vec::new();

        for id in ids {
            let (pos, health_frac, terrain_y_fallback) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                (
                    obj.get_position(),
                    obj.health_fraction(),
                    obj.get_position().y,
                )
            };

            let eps = 4.0;
            let h0 = self.terrain_height_at(pos).unwrap_or(terrain_y_fallback);
            let h_x = self
                .terrain_height_at(Vec3::new(pos.x + eps, pos.y, pos.z))
                .unwrap_or(h0);
            let h_z = self
                .terrain_height_at(Vec3::new(pos.x, pos.y, pos.z + eps))
                .unwrap_or(h0);
            let mut normal = Vec3::new(h0 - h_x, eps, h0 - h_z);
            if normal.length_squared() < 1.0e-8 {
                normal = Vec3::Y;
            } else {
                normal = normal.normalize();
            }

            // Sample terrain heights without holding object borrows.
            let gh_samples: Vec<(f32, f32, f32)> = {
                let mut samples = Vec::new();
                if let Some(terrain) = self.terrain.as_ref() {
                    // Pre-sample a small grid around current pos for tick closure.
                    for dx in [-8.0_f32, 0.0, 8.0] {
                        for dz in [-8.0_f32, 0.0, 8.0] {
                            let x = pos.x + dx;
                            let z = pos.z + dz;
                            samples.push((x, z, terrain.height_at_world(Vec3::new(x, 0.0, z))));
                        }
                    }
                }
                samples
            };
            let flat_y = h0;
            let ground_height_at = move |x: f32, z: f32| -> f32 {
                if gh_samples.is_empty() {
                    return flat_y;
                }
                let mut best = flat_y;
                let mut best_d = f32::MAX;
                for &(sx, sz, h) in &gh_samples {
                    let d = (sx - x) * (sx - x) + (sz - z) * (sz - z);
                    if d < best_d {
                        best_d = d;
                        best = h;
                    }
                }
                best
            };

            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if obj.tensile_formation.is_none() {
                continue;
            }
            // Init links before exclusive data tick.
            let needs_init = obj
                .tensile_formation
                .as_ref()
                .map(|d| !d.links_inited)
                .unwrap_or(false);
            if needs_init {
                if let Some(data) = obj.tensile_formation.as_mut() {
                    data.init_links(id, pos, &members);
                }
            }
            let result = {
                let data = obj.tensile_formation.as_mut().unwrap();
                data.tick(frame, pos, health_frac, normal, &ground_height_at, &members)
            };
            if result.became_enabled {
                self.tensile_formation_reg.record_enable();
            }
            if result.play_crack {
                crack_events.push(pos);
            }
            if result.propagate {
                propagate_centers.push(result.new_pos.unwrap_or(pos));
            }
            if result.became_rubble {
                rubble_ids.push(id);
            }
            if result.slid {
                self.tensile_formation_reg.record_slide();
            }
            if let Some(np) = result.new_pos {
                position_updates.push((id, np));
            }
        }

        for (id, np) in position_updates {
            if let Some(o) = self.objects.get_mut(&id) {
                o.set_position(np);
            }
        }

        for pos in crack_events {
            self.tensile_formation_reg.record_crack();
            self.queue_audio_event(
                AudioEventRequest::new(TENSILE_CRACK_SOUND)
                    .with_position(pos)
                    .with_priority(120),
            );
        }

        for center in propagate_centers {
            self.tensile_formation_reg.record_propagate();
            let hurt: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_alive() && o.has_tensile_formation())
                .filter(|(_, o)| {
                    let p = o.get_position();
                    let dx = p.x - center.x;
                    let dz = p.z - center.z;
                    (dx * dx + dz * dz).sqrt() <= TENSILE_PROPAGATE_RADIUS
                })
                .map(|(hid, _)| *hid)
                .collect();
            for hid in hurt {
                if let Some(o) = self.objects.get_mut(&hid) {
                    let max_h = o.health.maximum.max(o.max_health).max(1.0);
                    let cap = max_h * TENSILE_BODY_DAMAGED_HEALTH_FRAC;
                    if o.health.current > cap {
                        o.health.current = cap;
                    }
                    if let Some(tf) = o.tensile_formation.as_mut() {
                        tf.set_enabled(true);
                    }
                }
            }
        }

        for id in rubble_ids {
            self.tensile_formation_reg.record_rubble();
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 749: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log;
                // non-authority path keeps host HP clear. Tensile rubble flags
                // remain host residual either way.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    crate::game_logic::host_damage_log::record(id, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                if let Some(tf) = o.tensile_formation.as_mut() {
                    tf.rubble = true;
                    tf.done = true;
                    tf.moving = false;
                    tf.freefall = false;
                    tf.post_collapse = true;
                }
            }
        }
    }

    pub(crate) fn tick_zone_damage_fields_sole(&mut self) {
        // Wave 825: post-writeback sole-tick for host zone/field damage residuals.
        self.update_scud_poison_zones();
        self.update_nuclear_tanks_radiation_zones();
        self.update_firewalls();
        self.update_inferno_fire_zones();
        self.update_spectre_orbit_fields();
        self.update_toxin_tractor_poison_zones();
        self.update_anthrax_toxin_fields();
        self.update_nuclear_radiation_fields();
        self.update_neutron_slow_death_fields();
        self.update_microwave_emitter_field();
        self.update_microwave_disable();
    }

    pub(in super::super) fn update_scud_poison_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                )
            })
            .collect();

        let plans = self
            .scud_poison_zones
            .plan_due_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        crate::game_logic::host_poisoned_behavior::poison_weapon_damage_type(),
                        plan.death_type,
                    );
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.scud_poison_zones.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.scud_poison_zones.prune_expired(frame);
    }

    /// Advance Nuke Cannon MediumRadiationField residual zones.
    pub(in super::super) fn update_nuke_cannon_radiation_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .nuke_cannon_residual
            .plan_due_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_radiation_field_tick(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.nuke_cannon_residual.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.nuke_cannon_residual.prune_expired(frame);
    }

    /// Apply Nuke Cannon primary residual: area shell + MediumRadiationField spawn.
    ///
    /// Returns (units_hit, any_destroyed).
    /// C++ NukeCannonShell DumbProjectile residual (Bezier flight + primary blast).
    pub fn spawn_nuke_cannon_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_nuke_cannon::{
            NUKE_CANNON_PROJECTILE, NUKE_SHELL_MAX_HEALTH, nuke_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(NUKE_CANNON_PROJECTILE) {
            let mut t = ThingTemplate::new(NUKE_CANNON_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(NUKE_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(NUKE_CANNON_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on NukeCannonGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_nuke_cannon::nuke_cannon_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.nuke_cannon_scatter_applied = self.nuke_cannon_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_nuke_cannon::nuke_cannon_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    // Secondary splash outer bound (60); outside = pure splash miss.
                    if dist > crate::game_logic::host_nuke_cannon::NUKE_CANNON_SECONDARY_RADIUS {
                        self.nuke_cannon_scatter_misses =
                            self.nuke_cannon_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 2.0;
        let pid = self.create_object(NUKE_CANNON_PROJECTILE, team, start)?;
        let frames = nuke_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.nuke_cannon_shell_projectile = true;
            o.nuke_shell_from = Some([start.x, start.y, start.z]);
            o.nuke_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.nuke_shell_launch_frame = Some(self.frame);
            o.nuke_shell_flight_frames = frames;
            o.note_producer(source_id);
            o.health.maximum = NUKE_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, NUKE_SHELL_MAX_HEALTH);
        }
        self.nuke_cannon_shells_spawned = self.nuke_cannon_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_nuke_cannon_shell_projectiles(&mut self) {
        use crate::game_logic::host_nuke_cannon::nuke_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.nuke_cannon_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Team, glam::Vec3)> = Vec::new();
        for id in flying {
            let (source, team, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .nuke_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .nuke_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.team,
                    from,
                    aim,
                    o.nuke_shell_launch_frame.unwrap_or(frame),
                    o.nuke_shell_flight_frames.max(1),
                )
            };
            // Prefer live producer team if available.
            let team = source
                .and_then(|sid| self.objects.get(&sid).map(|s| s.team))
                .unwrap_or(team);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = nuke_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, team, aim));
            }
        }
        for (id, source, team, pos) in impact {
            let shell_team = self.objects.get(&id).map(|o| o.team);
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
                o.nuke_cannon_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_nuke_cannon_primary_at(pos, source, team);
            self.mark_object_for_destruction(id, shell_team);
        }
    }

    pub fn honesty_nuke_cannon_shell_projectile_ok(&self) -> bool {
        self.nuke_cannon_shells_spawned > 0
    }

    /// Residual honesty: NukeCannon ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_nuke_cannon_scatter_ok(&self) -> bool {
        self.nuke_cannon_scatter_applied > 0 || self.nuke_cannon_scatter_misses > 0
    }
}
