#[path = "projectile_step.rs"]
mod projectile_step;

// C++ ownership: Weapon.cpp projectile resolution, damage ordering, and impact side effects.

impl Default for CombatSystem {
    fn default() -> Self {
        Self::new()
    }
}
fn projectile_goal_on_object(target: &Object) -> Vec3 {
    let pos = target.get_position();
    let mut goal = pos;
    let half = (*target.geometry_info()).max_height_above_position() * 0.5;
    goal.y += half;
    if pos.y < goal.y {
        goal.y += half;
    }
    goal
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {
            projectiles: HashMap::new(),
            fire_sound_loop_events: Default::default(),
            pending_projectiles: Vec::new(),
            next_projectile_id: ObjectId(100000), // Start high to avoid conflicts with objects
            impact_fx: Vec::new(),
            pending_under_attack: Vec::new(),
            pending_on_die: Vec::new(),
            fire_ocl: Vec::new(),
            historic_bonus: Default::default(),
            projectileless_delayed: Vec::new(),
        }
    }

    /// Reset all mutable combat state for this receiving match. C++
    /// GameLogic.cpp410–435 destroys projectile Objects and resets allocation;
    /// WeaponStore.cpp1627–1646 clears delayed damage and template history.
    /// Immutable weapon/projectile definitions live on GameLogic, not here.
    pub(crate) fn reset_for_new_game(&mut self) {
        *self = Self::new();
    }

    /// Discard accepted shots only when this owning game resets.
    pub(crate) fn reset_pending_projectiles(&mut self) {
        self.pending_projectiles.clear();
        self.fire_sound_loop_events.clear();
    }

    /// Snapshot active projectiles for PresentationFrame freeze (read-only).
    pub fn projectiles_snapshot(&self) -> Vec<&Projectile> {
        self.projectiles.values().collect()
    }

    /// Drain ProjectileDetonationFX residual events produced by the last update.
    pub fn take_impact_fx(&mut self) -> Vec<ProjectileImpactFx> {
        std::mem::take(&mut self.impact_fx)
    }

    /// Drain victims that took live projectile/area damage this pass.
    pub fn take_pending_under_attack(&mut self) -> Vec<ObjectId> {
        std::mem::take(&mut self.pending_under_attack)
    }
    /// Drain victims killed by projectile or splash this pass (once each).
    pub fn take_pending_on_die(&mut self) -> Vec<ObjectId> {
        std::mem::take(&mut self.pending_on_die)
    }

    pub(in crate::game_logic) fn note_kill_for_on_die(
        &mut self,
        id: ObjectId,
        before_hp: f32,
        killed: bool,
        now_alive: bool,
        already: bool,
    ) {
        let lethal = killed || (before_hp > 0.0 && !now_alive);
        if lethal && !already {
            self.pending_on_die.push(id);
        }
    }

    /// C++ Object.cpp:1847-1849: enqueue only when actual HP was dealt and
    /// the type is not DAMAGE_PENALTY / DAMAGE_HEALING. Local-player,
    /// sourcePlayerMask, and radar-data gates run in
    /// `GameLogic::try_under_attack_from_damage`.
    pub(in crate::game_logic) fn queue_under_attack_if_dealt(
        &mut self,
        victim_id: ObjectId,
        damage_type: DamageType,
        hp_lost: f32,
    ) {
        if hp_lost > 0.0 && !matches!(damage_type, DamageType::Penalty | DamageType::Healing) {
            self.pending_under_attack.push(victim_id);
        }
    }

    /// Drain FireOCL events emitted while pending shots were materialized.
    pub fn take_fire_ocl(&mut self) -> Vec<WeaponFireOcl> {
        std::mem::take(&mut self.fire_ocl)
    }

    pub fn projectile_count(&self) -> usize {
        self.projectiles.len()
    }

    pub fn projectile_mut(&mut self, id: ObjectId) -> Option<&mut Projectile> {
        self.projectiles.get_mut(&id)
    }

    /// Remove one projectile by id (GameWorld projectile-authority writeback).
    pub fn remove_projectile(&mut self, id: ObjectId) -> bool {
        self.projectiles.remove(&id).is_some()
    }

    /// Fire a projectile from one object to another
    pub fn fire_projectile(
        &mut self,
        shooter_pos: Vec3,
        target_pos: Vec3,
        weapon: &Weapon,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
        speed: f32,
    ) -> ObjectId {
        self.fire_projectile_ex(
            shooter_pos,
            target_pos,
            weapon,
            shooter_id,
            target_id,
            speed,
            false,
        )
    }

    /// Fire with explicit homing residual.
    pub fn fire_projectile_ex(
        &mut self,
        shooter_pos: Vec3,
        target_pos: Vec3,
        weapon: &Weapon,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
        speed: f32,
        is_homing: bool,
    ) -> ObjectId {
        let projectile_id = self.next_projectile_id;
        self.next_projectile_id = ObjectId(self.next_projectile_id.0 + 1);

        let mut projectile = Projectile::new(
            projectile_id,
            shooter_pos,
            target_pos,
            weapon.damage,
            // C++ WeaponTemplate ctor defaults m_damageType to DAMAGE_EXPLOSION
            // (Weapon.cpp:249); the host Weapon payload carries no authored
            // type, so the direct-fire default is Explosion, not Bullet.
            DamageType::Explosive,
            shooter_id,
            target_id,
        );

        // C++ radius damage residual from WeaponTemplate splash/radius.
        projectile.explosion_radius = weapon.splash_radius.max(0.0);
        projectile.is_homing = is_homing && !Projectile::is_instant_speed(speed);

        if Projectile::is_instant_speed(speed) {
            // Laser / hitscan residual: spawn already at impact for same-frame resolve.
            projectile.speed = 0.0;
            projectile.velocity = Vec3::ZERO;
            projectile.position = target_pos;
            projectile.target_position = target_pos;
        } else {
            let spd = if speed > 0.0 { speed } else { 200.0 };
            let dir = (target_pos - shooter_pos).normalize_or_zero();
            projectile.speed = spd;
            projectile.velocity = dir * spd;
        }

        self.projectiles.insert(projectile_id, projectile);

        projectile_id
    }

    /// Update all projectiles

    fn maybe_record_historic_bonus(
        historic_bonus: &mut crate::game_logic::host_historic_bonus::HostHistoricBonus,
        frame: u32,
        historic_damage_limit: u32,
        projectile: &Projectile,
        impact_pos: Vec3,
        objects: HostObjectRead<'_>,
    ) {
        if projectile.historic_bonus_count <= 0 {
            return;
        }
        let peel = crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel {
            time_frames: projectile.historic_bonus_time_frames,
            count: projectile.historic_bonus_count,
            radius: projectile.historic_bonus_radius,
            bonus_weapon: projectile.historic_bonus_weapon.clone(),
        };
        if !peel.is_active() {
            return;
        }
        let team = objects
            .lookup(&projectile.shooter_id)
            .map(|o| o.team)
            .unwrap_or(crate::game_logic::Team::Neutral);
        let key = if projectile.historic_weapon_key.is_empty() {
            "weapon"
        } else {
            projectile.historic_weapon_key.as_str()
        };
        let _ = historic_bonus.record_impact(
            frame,
            historic_damage_limit,
            key,
            &peel,
            impact_pos,
            projectile.shooter_id,
            team,
        );
    }

    pub fn update_projectiles(
        &mut self,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        health_events: &mut crate::game_logic::HostHealthEvents,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_countermeasures(
            dt,
            objects,
            None,
            0,
            health_events,
            ownership_events,
        )
    }

    /// Flight integrate only (lifetime + pose). Hit/detonation behavior is
    /// separate, so this helper cannot silently discard a parsed expiry.
    pub fn integrate_projectiles_only(&mut self, dt: f32) -> usize {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            1.0 / 30.0
        };
        let ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();
        let mut stepped = 0usize;
        for id in ids {
            let Some(p) = self.projectiles.get_mut(&id) else {
                continue;
            };
            // This pose-only caller has no target-status input. Defer parsed
            // target-loss and detonation behavior to the complete combat pass.
            if p.update(dt, true) == ProjectileStep::Alive {
                stepped += 1;
            }
        }
        stepped
    }

    /// Refresh homing aim points from registered object positions.
    pub fn refresh_homing_aims(&mut self, objects: HostObjectRead<'_>) {
        for p in self.projectiles.values_mut() {
            if !p.is_homing {
                continue;
            }
            if let Some(tid) = p.target_id {
                if let Some(tgt) = objects.lookup(&tid) {
                    p.target_position = tgt.get_position();
                }
            }
        }
    }

    fn splash_area_event(
        projectile: &Projectile,
        objects: HostObjectRead<'_>,
        position: Vec3,
    ) -> DamageEvent {
        let live = objects.lookup(&projectile.shooter_id);
        // C++ fireWeaponTemplate: DamageDealtAtSelfPosition → damageID=INVALID,
        // damagePos=sourcePos (Weapon.cpp:1008, 1035). Stay off cone field.
        let at_self =
            crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name(
                &projectile.historic_weapon_key,
            );
        let position = if at_self {
            live.map(|o| o.get_position()).unwrap_or(position)
        } else {
            position
        };
        let primary_victim = if at_self { None } else { projectile.target_id };
        DamageEvent::Area {
            position,
            damage: projectile.damage,
            damage_type: projectile.damage_type,
            death_type: projectile.death_type,

            radius: projectile.explosion_radius,
            shooter_id: projectile.shooter_id,
            secondary_damage: projectile.secondary_damage,
            secondary_radius: projectile.secondary_damage_radius,
            shock_wave_amount: projectile.shock_wave_amount,
            shock_wave_radius: projectile.shock_wave_radius,
            shock_wave_taper_off: projectile.shock_wave_taper_off,
            radius_damage_affects: projectile.radius_damage_affects,
            shooter_team: live.map(|o| o.team).unwrap_or(projectile.source_team),
            shooter_owner_player_id: live
                .and_then(|o| o.owner_player_id)
                .or(projectile.source_owner_player_id),
            shooter_team_instance_name: live
                .map(|o| o.team_instance_name.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| projectile.source_team_instance_name.clone()),
            shooter_template: live.map(|o| o.template_name.clone()).unwrap_or_default(),
            primary_victim,
            radius_damage_angle: leftover_radius_damage_angle(&projectile.historic_weapon_key),
        }
    }

    /// C++ MissileAIUpdate::detonate: after handleProjectileDetonation, MissileCallsOnDie
    /// applies UNRESISTABLE / DETONATED / maxHealth to the **projectile** so its Die
    /// modules run. Victim splash keeps the weapon DeathType.
    fn queue_missile_calls_on_die(
        impact_fx: &mut Vec<ProjectileImpactFx>,
        projectile: &Projectile,
    ) {
        if !projectile.die_on_detonate {
            return;
        }
        let name = projectile.projectile_object_name.trim();
        if name.is_empty() {
            return;
        }
        let mut fx_name = String::new();
        if let Some(mut fx) =
            crate::game_logic::host_fx_list_die::fx_list_die_config_for_template(name)
        {
            let hits = fx.collect_applicable(
                &[],
                crate::game_logic::host_usa_pilot::HostDeathType::Detonated,
            );
            if let Some((Some(name), _)) = hits.into_iter().next() {
                fx_name = name;
            }
        }
        let ocl_name =
            crate::game_logic::host_create_object_die::create_object_die_config_for_template(name)
                .and_then(|mut cod| {
                    let _ = cod.on_die();
                    if cod.ocl_name.trim().is_empty() {
                        None
                    } else {
                        Some(cod.ocl_name)
                    }
                })
                .unwrap_or_default();
        if fx_name.is_empty() && ocl_name.is_empty() {
            return;
        }
        impact_fx.push(ProjectileImpactFx {
            position: projectile.position,
            shooter_id: projectile.shooter_id,
            target_id: projectile.target_id,
            detonation_fx_name: fx_name,
            detonation_ocl_name: ocl_name,
            source_team: projectile.source_team,
            source_veterancy: projectile.source_veterancy,
            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
            source_velocity: projectile.velocity,
        });
    }

    /// C++ `FXList::doFXObj(d->m_garrisonHitKillFX, other, NULL)` on the
    /// building. Must not go through `ProjectileImpactFx` / Weapon.cpp
    /// `doFXPos` detonation (shooter matrix + overrideRadius).
    fn play_garrison_hit_kill_fx(
        objects: HostObjectRead<'_>,
        building_id: ObjectId,
        fx_name: &str,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
    ) {
        if !fx_name.is_empty() {
            if let Some(building) = objects.lookup(&building_id) {
                fx(fx_name, ProjectileFxObject::Object(building));
            }
        }
    }

    /// Projectile step with optional America Countermeasures diversion residual.
    pub fn update_projectiles_with_countermeasures(
        &mut self,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_relationships(
            dt,
            objects,
            countermeasures,
            frame,
            None,
            None,
            health_events,
            ownership_events,
        )
    }

    /// Projectile step that applies RadiusDamageAffects via GameWorld player relationships.

    pub fn update_projectiles_with_relationships(
        &mut self,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
        team_factory: Option<&gamelogic::team::TeamFactoryHandle>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_relationships_and_terrain(
            dt,
            objects,
            countermeasures,
            frame,
            players,
            team_factory,
            health_events,
            None,
            ownership_events,
        )
    }

    pub(crate) fn update_projectiles_with_relationships_and_terrain(
        &mut self,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
        team_factory: Option<&gamelogic::team::TeamFactoryHandle>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) -> Vec<ObjectId> {
        let ids = self.projectile_update_ids();
        let limit = game_engine::common::global_data::read().historic_damage_limit;
        let mut countermeasures = countermeasures;
        let mut retired = Vec::new();
        for id in ids {
            let (damage_events, removals) = self.prepare_projectile_impact(
                id,
                dt,
                objects,
                countermeasures.as_deref_mut(),
                frame,
                terrain,
                ownership_events,
                limit,
            );
            self.apply_damage_events(
                &damage_events,
                objects,
                players,
                team_factory,
                frame,
                health_events,
                ownership_events,
            );
            retired.extend(removals);
        }
        self.retire_projectile_impacts(&retired);
        retired
    }

    /// Test-only flight collection for fixtures that supply no Body callbacks.
    #[cfg(test)]
    pub(in crate::game_logic) fn prepare_projectile_impacts(
        &mut self,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) -> (Vec<DamageEvent>, Vec<ObjectId>) {
        let ids = self.projectile_update_ids();
        let limit = game_engine::common::global_data::read().historic_damage_limit;
        let mut events = Vec::new();
        let mut retired = Vec::new();
        for id in ids {
            let (hits, removals) = self.prepare_projectile_impact(
                id,
                dt,
                objects,
                countermeasures.as_deref_mut(),
                frame,
                terrain,
                ownership_events,
                limit,
            );
            events.extend(hits);
            retired.extend(removals);
        }
        (events, retired)
    }

    /// Existing arena encounter only; actual UpdateModule admission is hq-om90a.
    pub(in crate::game_logic) fn projectile_update_ids(&self) -> Vec<ObjectId> {
        self.projectiles.keys().copied().collect()
    }

    /// Existing physical removal boundary after all selected callbacks complete.
    pub(in crate::game_logic) fn retire_projectile_impacts(
        &mut self,
        projectiles_to_remove: &[ObjectId],
    ) {
        // CPP destroyObject marks destruction before later physical removal.
        // Full per-module deletion phases remain tracked in hq-om90a.
        // The observation importer discovers retirement from this canonical map.
        for proj_id in projectiles_to_remove {
            self.projectiles.remove(proj_id);
        }
    }

    pub(in crate::game_logic) fn apply_damage_events(
        &mut self,
        damage_events: &[DamageEvent],
        objects: &mut crate::game_logic::HostObjectStore,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
        team_factory: Option<&gamelogic::team::TeamFactoryHandle>,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) {
        for hit in damage_events {
            match hit {
                DamageEvent::Direct {
                    target_id,
                    damage,
                    damage_type,
                    death_type,
                    shooter_id,
                    ..
                } => {
                    let context = crate::game_logic::object::DamageHitContext::new(
                        objects.get(shooter_id),
                        None,
                        *damage_type,
                    );
                    if let Some(target) = objects.get_mut(target_id) {
                        let before = target.health.current;
                        let destroyed = target.take_damage_with_context(
                            *damage,
                            Some(*shooter_id),
                            *damage_type,
                            *death_type,
                            None,
                            frame,
                            &context,
                            health_events,
                            Some(ownership_events),
                        );
                        let hp_lost = (before - target.health.current).max(0.0);
                        self.note_kill_for_on_die(
                            *target_id,
                            before,
                            destroyed,
                            target.health.is_alive(),
                            target.status.on_die_started,
                        );
                        self.queue_under_attack_if_dealt(*target_id, *damage_type, hp_lost);
                        if destroyed {
                            log::debug!(
                                "Projectile destroyed object {} (damage: {:.1}, type: {:?})",
                                target_id,
                                damage,
                                damage_type,
                            );
                        }
                    }
                }
                DamageEvent::Area {
                    position,
                    damage,
                    damage_type,
                    death_type,
                    radius,
                    secondary_damage,
                    secondary_radius,
                    shock_wave_amount,
                    shock_wave_radius,
                    shock_wave_taper_off,
                    shooter_id,
                    radius_damage_affects,
                    shooter_owner_player_id,
                    shooter_team_instance_name,
                    shooter_template,
                    primary_victim,
                    radius_damage_angle,
                    shooter_team,
                } => {
                    let context = crate::game_logic::object::DamageHitContext::new(
                        objects.get(shooter_id),
                        None,
                        *damage_type,
                    );
                    let primary_r = *radius;
                    let secondary_r = (*secondary_radius).max(0.0);
                    let dual = secondary_r > primary_r + 1e-3 && *secondary_damage > 0.0;
                    let outer = if dual { secondary_r } else { primary_r };
                    let shock_r = (*shock_wave_radius).max(0.0);
                    let shock_amt = (*shock_wave_amount).max(0.0);
                    let shock_taper = (*shock_wave_taper_off).clamp(0.0, 1.0);
                    let shooter_pos = objects.get(shooter_id).map(|s| s.get_position());
                    let shooter_producer = objects.get(shooter_id).and_then(|s| s.producer_id);
                    let shooter_dir_xz = objects.get(shooter_id).map(|s| s.unit_direction_xz());
                    for (vid, obj) in objects.registered_mut() {
                        let op = obj.get_position();
                        let dist = splash_from_bounding_sphere_3d(
                            *position,
                            op,
                            victim_splash_sphere_radius(obj),
                        );
                        if dist > outer {
                            continue;
                        }
                        let is_primary = *primary_victim == Some(*vid);
                        let kills_self = (*radius_damage_affects
                            & crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF)
                            != 0
                            && *vid == *shooter_id;
                        if !is_primary && !kills_self {
                            let airborne = obj.is_significantly_above_terrain();
                            let same_tmpl =
                                crate::game_logic::weapon_bootstrap::splash_templates_equivalent(
                                    shooter_template,
                                    &obj.template_name,
                                );
                            let relationship = match players {
                                Some(map) if team_factory.is_some() => {
                                    crate::game_logic::GameLogic::object_relationship_from_owners(
                                        team_factory.expect("guarded Some"),
                                        map,
                                        obj.owner_player_id,
                                        &obj.team_instance_name,
                                        *shooter_owner_player_id,
                                        shooter_team_instance_name,
                                    )
                                }
                                // C++ curVictim->getRelationship(source) is
                                // ownership-driven; the live host falls back to
                                // the frozen launch teams when no player
                                // registry is wired into this combat pass.
                                Some(map) if obj.owner_player_id == *shooter_owner_player_id => {
                                    gamelogic::common::Relationship::Allies
                                }
                                Some(map) => {
                                    let _ = map;
                                    if obj.team == *shooter_team {
                                        gamelogic::common::Relationship::Allies
                                    } else {
                                        gamelogic::common::Relationship::Neutral
                                    }
                                }
                                None if obj.team == *shooter_team => {
                                    gamelogic::common::Relationship::Allies
                                }
                                None => gamelogic::common::Relationship::Neutral,
                            };
                            let allowed =
                                crate::game_logic::weapon_bootstrap::radius_damage_affects_victim(
                                    *radius_damage_affects,
                                    relationship,
                                    *shooter_id,
                                    *vid,
                                    shooter_producer,
                                    airborne,
                                    same_tmpl,
                                );
                            if !allowed {
                                continue;
                            }
                        }
                        if !leftover_radius_damage_cone_allows(
                            *radius_damage_angle,
                            shooter_pos,
                            shooter_dir_xz,
                            op,
                        ) {
                            continue;
                        }
                        if dist <= outer {
                            let area_damage = if kills_self && !is_primary {
                                HUGE_DAMAGE_AMOUNT
                            } else if dist <= primary_r {
                                *damage
                            } else if secondary_r > 0.0 {
                                *secondary_damage
                            } else {
                                0.0
                            };
                            if area_damage > 0.0 {
                                let before = obj.health.current;
                                let destroyed = obj.take_damage_with_context(
                                    area_damage,
                                    Some(*shooter_id),
                                    *damage_type,
                                    *death_type,
                                    None,
                                    frame,
                                    &context,
                                    health_events,
                                    Some(ownership_events),
                                );
                                let hp_lost = (before - obj.health.current).max(0.0);
                                self.note_kill_for_on_die(
                                    *vid,
                                    before,
                                    destroyed,
                                    obj.health.is_alive(),
                                    obj.status.on_die_started,
                                );
                                self.queue_under_attack_if_dealt(*vid, *damage_type, hp_lost);
                            }
                        }
                        if shock_amt > 0.0 && shock_r > 0.0 {
                            let origin = shooter_pos.unwrap_or(*position);
                            if let Some(force) =
                                crate::game_logic::weapon_bootstrap::compute_shock_wave_force(
                                    origin,
                                    op,
                                    shock_amt,
                                    shock_r,
                                    shock_taper,
                                )
                            {
                                let _ = obj.apply_shock_wave_impulse(force);
                            }
                        }
                    }
                }
            }
        }
    }

    fn apply_projectileless_delayed_shot(
        &mut self,
        shot: &LiveProjectilelessDelayedDamage,
        objects: &mut crate::game_logic::HostObjectStore,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
        frame: u32,
        historic_damage_limit: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) {
        let events = self.prepare_projectileless_delayed_shot(
            shot,
            objects.read(),
            frame,
            historic_damage_limit,
        );
        self.apply_damage_events(
            &events,
            objects,
            players,
            None,
            frame,
            health_events,
            ownership_events,
        );
    }

    /// Prepare just this impact; the owner completes it before preparing another.
    pub(in crate::game_logic) fn prepare_projectileless_delayed_shot(
        &mut self,
        shot: &LiveProjectilelessDelayedDamage,
        objects: HostObjectRead<'_>,
        frame: u32,
        historic_damage_limit: u32,
    ) -> Vec<DamageEvent> {
        let p = &shot.pending;
        let mut proj = Projectile::new(
            ObjectId(0),
            p.shooter_pos,
            shot.damage_pos,
            p.damage,
            p.damage_type,
            p.shooter_id,
            shot.damage_id,
        );
        proj.death_type = p.death_type;
        proj.explosion_radius = p.splash_radius.max(0.0);
        proj.secondary_damage = p.secondary_damage;
        proj.secondary_damage_radius = p.secondary_damage_radius;
        proj.shock_wave_amount = p.shock_wave_amount;
        proj.shock_wave_radius = p.shock_wave_radius;
        proj.shock_wave_taper_off = p.shock_wave_taper_off;
        proj.radius_damage_affects = p.radius_damage_affects;
        proj.source_team = p
            .source_context
            .map(|c| c.source_team)
            .unwrap_or(crate::game_logic::Team::Neutral);
        proj.source_owner_player_id = p
            .source_context
            .and_then(|c| c.source_owner_player_id)
            .or_else(|| {
                objects
                    .lookup(&p.shooter_id)
                    .and_then(|o| o.owner_player_id)
            });
        proj.source_team_instance_name = objects
            .lookup(&p.shooter_id)
            .map(|o| o.team_instance_name.clone())
            .unwrap_or_default();
        proj.source_veterancy = p
            .source_context
            .map(|c| c.source_veterancy)
            .unwrap_or(crate::game_logic::VeterancyLevel::Rookie);
        proj.historic_weapon_key = p.historic_weapon_key.clone();
        proj.historic_bonus_time_frames = p.historic_bonus_time_frames;
        proj.historic_bonus_count = p.historic_bonus_count;
        proj.historic_bonus_radius = p.historic_bonus_radius;
        proj.historic_bonus_weapon = p.historic_bonus_weapon.clone();
        proj.detonation_fx_name = p.detonation_fx_name.clone();
        proj.detonation_ocl_name = p.detonation_ocl_name.clone();

        Self::maybe_record_historic_bonus(
            &mut self.historic_bonus,
            frame,
            historic_damage_limit,
            &proj,
            shot.damage_pos,
            objects,
        );
        let mut events = Vec::new();
        if proj.explosion_radius > 0.0 || proj.secondary_damage_radius > 0.0 {
            events.push(Self::splash_area_event(&proj, objects, shot.damage_pos));
        } else if let Some(target_id) = shot.damage_id {
            events.push(DamageEvent::Direct {
                target_id,
                position: shot.damage_pos,
                damage: proj.damage,
                damage_type: proj.damage_type,
                death_type: proj.death_type,
                shooter_id: proj.shooter_id,
            });
        }
        if !proj.detonation_fx_name.is_empty() || !proj.detonation_ocl_name.is_empty() {
            self.impact_fx.push(ProjectileImpactFx {
                position: shot.damage_pos,
                shooter_id: proj.shooter_id,
                target_id: shot.damage_id,
                detonation_fx_name: proj.detonation_fx_name.clone(),
                detonation_ocl_name: proj.detonation_ocl_name.clone(),
                source_team: proj.source_team,
                source_veterancy: proj.source_veterancy,
                source_orientation: 0.0,
                source_velocity: Vec3::ZERO,
            });
        }
        events
    }

    /// Get all active projectiles
    pub fn get_projectiles(&self) -> &HashMap<ObjectId, Projectile> {
        &self.projectiles
    }

    pub(in crate::game_logic) fn take_historic_bonus_firestorms(
        &mut self,
    ) -> Vec<crate::game_logic::host_historic_bonus::PendingHistoricFirestorm> {
        self.historic_bonus.drain_pending_firestorms()
    }

    pub(in crate::game_logic) fn historic_bonus_honesty(
        &self,
    ) -> crate::game_logic::host_historic_bonus::HostHistoricBonusHonesty {
        self.historic_bonus.honesty_snapshot()
    }

    /// Clear all projectiles
    pub fn clear(&mut self) {
        self.projectiles.clear();
    }
}

/// C++ Weapon.cpp:1144-1155 report at launch; MissileAIUpdate decoy when the timer expires.
fn apply_countermeasure_report_and_decoy(
    projectile: &mut Projectile,
    proj_id: ObjectId,
    objects: HostObjectRead<'_>,
    reg: &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
    frame: u32,
) {
    use crate::game_logic::host_countermeasures::{
        MISSILE_DECOY_DELAY_FRAMES, aircraft_has_countermeasures_upgrade,
        calculate_countermeasure_to_divert_to, report_missile_for_countermeasures_named,
        victim_locomotor_is_supersonic,
    };

    if !projectile.cm_reported {
        projectile.cm_reported = true;
        if projectile.is_small_missile {
            if let Some(tid) = projectile.target_id {
                if let Some(target) = objects.lookup(&tid) {
                    let has_cm = aircraft_has_countermeasures_upgrade(&target.applied_upgrades);
                    let airborne =
                        target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;
                    let supersonic =
                        victim_locomotor_is_supersonic(target.get_cur_locomotor_set_token());
                    if has_cm && airborne && !supersonic {
                        let tmpl = target.template_name.clone();
                        if report_missile_for_countermeasures_named(
                            reg,
                            tid,
                            proj_id,
                            frame,
                            true,
                            Some(tmpl.as_str()),
                        ) {
                            projectile.frames_till_decoyed =
                                frame.saturating_add(MISSILE_DECOY_DELAY_FRAMES);
                        }
                    }
                }
            }
        }
    }

    if projectile.frames_till_decoyed > 0 && projectile.frames_till_decoyed <= frame {
        projectile.frames_till_decoyed = 0;
        projectile.no_damage = true;
        if let Some(victim_id) = projectile.target_id {
            let aircraft_xz = objects.lookup(&victim_id).map(|o| {
                let p = o.get_position();
                (p.x, p.z)
            });
            if let Some(axz) = aircraft_xz {
                let flare_xz: Vec<(ObjectId, f32, f32)> = objects
                    .registered()
                    .filter(|(_, o)| {
                        o.countermeasure_flare && o.producer_id == Some(victim_id) && o.is_alive()
                    })
                    .map(|(id, o)| {
                        let p = o.get_position();
                        (*id, p.x, p.z)
                    })
                    .collect();
                if let Some(fid) =
                    calculate_countermeasure_to_divert_to(reg, victim_id, axz, &flare_xz)
                {
                    projectile.target_id = Some(fid);
                    projectile.is_homing = true;
                    if let Some(flare) = objects.lookup(&fid) {
                        projectile.target_position = flare.get_position();
                    }
                }
            }
        }
    }
}

/// C++ `HUGE_DAMAGE_AMOUNT` (Damage.h) for `WEAPON_KILLS_SELF`.
const HUGE_DAMAGE_AMOUNT: f32 = 999_999.0;

/// C++ `DAMAGE_RANGE_CALC_TYPE = FROM_BOUNDINGSPHERE_3D` (Weapon.cpp:70):
/// center-to-center minus victim bounding-sphere radius, clamped at 0.
pub(crate) fn splash_from_bounding_sphere_3d(
    center: Vec3,
    victim_pos: Vec3,
    victim_sphere: f32,
) -> f32 {
    let center_dist = victim_pos.distance(center);
    let sphere = victim_sphere.max(0.0);
    if center_dist <= sphere {
        0.0
    } else {
        center_dist - sphere
    }
}

pub(crate) fn victim_splash_sphere_radius(obj: &Object) -> f32 {
    let geom = &(*obj.geometry_info());
    crate::game_logic::host_battlemaster::leftover_horde_bounding_sphere_radius(
        geom.authored,
        geom.bounding_sphere_radius(),
        obj.selection_radius,
    )
}

/// Leftover `WeaponTemplate::radius_damage_angle`. Missing name / store miss → C++ PI (full circle).
fn leftover_radius_damage_angle(weapon_name: &str) -> f32 {
    if weapon_name.is_empty() {
        return std::f32::consts::PI;
    }
    crate::game_logic::weapon_bootstrap::with_host_weapon_store(|store| {
        store
            .find_weapon_template(weapon_name)
            .map(|wt| wt.radius_damage_angle)
    })
    .ok()
    .flatten()
    .unwrap_or(std::f32::consts::PI)
}

/// Leftover `deal_damage_internal` cone (Weapon.cpp:1393-1408).
/// `allowed_angle < PI` gates splash to the source facing cone; missing source skips the victim.
fn leftover_radius_damage_cone_allows(
    allowed_angle: f32,
    source_pos: Option<Vec3>,
    source_dir_xz: Option<(f32, f32)>,
    victim_pos: Vec3,
) -> bool {
    if allowed_angle >= std::f32::consts::PI {
        return true;
    }
    let Some(source_pos) = source_pos else {
        return false;
    };
    let Some((fx, fz)) = source_dir_xz else {
        return false;
    };
    // Live XZ ground: leftover/C++ source X-vector is horizontal facing (Y-up host).
    let source_dir = Vec3::new(fx, 0.0, fz);
    let damage_dir = victim_pos - source_pos;
    let source_n = source_dir.normalize_or_zero();
    let damage_n = damage_dir.normalize_or_zero();
    source_n.dot(damage_n) >= allowed_angle.cos()
}
