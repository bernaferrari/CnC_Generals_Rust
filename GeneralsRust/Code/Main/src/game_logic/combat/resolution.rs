// C++ ownership: Weapon.cpp projectile resolution, damage ordering, and impact side effects.

impl Default for CombatSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {
            projectiles: HashMap::new(),
            next_projectile_id: ObjectId(100000), // Start high to avoid conflicts with objects
            impact_fx: Vec::new(),
            pending_under_attack: Vec::new(),
            fire_ocl: Vec::new(),
        }
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

    /// C++ Object.cpp:1847-1849: enqueue only when actual HP was dealt and
    /// the type is not DAMAGE_PENALTY / DAMAGE_HEALING. Local-player,
    /// sourcePlayerMask, and radar-data gates run in
    /// `GameLogic::try_under_attack_from_damage`.
    fn queue_under_attack_if_dealt(
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
        projectile: &Projectile,
        impact_pos: Vec3,
        objects: &HashMap<ObjectId, Object>,
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
            .get(&projectile.shooter_id)
            .map(|o| o.team)
            .unwrap_or(crate::game_logic::Team::Neutral);
        let key = if projectile.historic_weapon_key.is_empty() {
            "weapon"
        } else {
            projectile.historic_weapon_key.as_str()
        };
        let _ = crate::game_logic::host_historic_bonus::record_impact(
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
        objects: &mut HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_countermeasures(dt, objects, None, 0)
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

    /// Refresh homing aim points from live object positions.
    pub fn refresh_homing_aims(&mut self, objects: &HashMap<ObjectId, Object>) {
        for p in self.projectiles.values_mut() {
            if !p.is_homing {
                continue;
            }
            if let Some(tid) = p.target_id {
                if let Some(tgt) = objects.get(&tid) {
                    if tgt.is_alive() {
                        p.target_position = tgt.get_position();
                    }
                }
            }
        }
    }

    fn splash_area_event(
        projectile: &Projectile,
        objects: &HashMap<ObjectId, Object>,
        position: Vec3,
    ) -> DamageEvent {
        let live = objects.get(&projectile.shooter_id);
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
        objects: &HashMap<ObjectId, Object>,
        building_id: ObjectId,
        fx_name: &str,
    ) {
        if fx_name.is_empty() {
            return;
        }
        if let Some(building) = objects.get(&building_id) {
            crate::game_logic::publish_host_fx_object(
                building_id.0,
                building.get_position(),
                building.get_orientation(),
                building.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            );
        }
        let _ = crate::game_logic::dispatch_fx_list_at_object(fx_name, building_id.0, None);
    }

    /// Projectile step with optional America Countermeasures diversion residual.
    pub fn update_projectiles_with_countermeasures(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_relationships(dt, objects, countermeasures, frame, None)
    }

    /// Projectile step that applies RadiusDamageAffects via GameWorld player relationships.
    pub fn update_projectiles_with_relationships(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
    ) -> Vec<ObjectId> {
        let projectile_ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();

        // Process projectile updates
        let mut damage_events = Vec::new();
        let mut projectiles_to_remove = Vec::new();

        for proj_id in projectile_ids {
            if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
                if let Some(reg) = countermeasures.as_mut() {
                    apply_countermeasure_report_and_decoy(projectile, proj_id, objects, reg, frame);
                }
                let target_is_live = projectile
                    .target_id
                    .map(|target_id| objects.get(&target_id).is_some_and(Object::is_alive))
                    .unwrap_or(true);
                // Homing residual: refresh aim point from live target before step.
                if projectile.is_homing {
                    if let Some(tid) = projectile.target_id {
                        if let Some(tgt) = objects.get(&tid) {
                            if tgt.is_alive() {
                                projectile.target_position = tgt.get_position();
                            }
                        }
                    }
                }
                if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb)) =
                    projectile.flight.clone()
                {
                    if dumb.flight_path_adjust_per_frame > 0.0 {
                        if let Some(tid) = projectile.target_id {
                            if let Some(tgt) = objects.get(&tid) {
                                if tgt.is_alive() {
                                    crate::game_logic::weapon_bootstrap::adjust_flight_path_end(
                                        &mut projectile.flight_runtime,
                                        &dumb,
                                        tgt.get_position(),
                                    );
                                    projectile.target_position = projectile.flight_runtime.path_end;
                                }
                            }
                        }
                    }
                }
                match projectile.update(dt, target_is_live) {
                    ProjectileStep::Alive => {
                        if !projectile.is_warhead_armed() {
                            continue;
                        }
                    }
                    ProjectileStep::Hold => {
                        // C++ MissileAIUpdate::KILL_SELF holds for the parsed
                        // contrail delay, with no impact FX/OCL or damage.
                        continue;
                    }
                    ProjectileStep::Remove => {
                        projectiles_to_remove.push(proj_id);
                        continue;
                    }
                    step @ (ProjectileStep::Detonate | ProjectileStep::DetonateAndHold) => {
                        // C++ DumbProjectileBehavior lifespan and
                        // MissileAIUpdate DetonateOnNoFuel both call the
                        // detonation weapon at the projectile's current pose.
                        // A zero-radius weapon has no guessed direct victim.
                        let impact = projectile.position;
                        Self::maybe_record_historic_bonus(projectile, impact, objects);
                        if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                            damage_events
                                .push(Self::splash_area_event(projectile, objects, impact));
                        }
                        if !projectile.detonation_fx_name.is_empty()
                            || !projectile.detonation_ocl_name.is_empty()
                        {
                            self.impact_fx.push(ProjectileImpactFx {
                                position: impact,
                                shooter_id: projectile.shooter_id,
                                target_id: None,
                                detonation_fx_name: projectile.detonation_fx_name.clone(),
                                detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                source_team: projectile.source_team,
                                source_veterancy: projectile.source_veterancy,
                                source_orientation: projectile
                                    .velocity
                                    .z
                                    .atan2(projectile.velocity.x),
                                source_velocity: projectile.velocity,
                            });
                        }
                        Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                        if step == ProjectileStep::Detonate {
                            projectiles_to_remove.push(proj_id);
                        }
                        continue;
                    }
                }

                // Intervening structure residual: ballistic shells impact the first
                // constructed building whose footprint contains the projectile.
                // Gated by Weapon.ini ProjectileCollidesWith leftover mask.
                // C++ Weapon.cpp:716-721 — same-controller structures only if
                // CONTROLLED_STRUCTURES; STRUCTURES is everyone else.
                // Skip intended target (handled below) and shooter.
                let mut hit_structure: Option<ObjectId> = None;
                let collides_structures =
                    crate::game_logic::weapon_bootstrap::projectile_collides_structures(
                        projectile.projectile_collides,
                    );
                if collides_structures {
                    let shooter = projectile.shooter_id;
                    let intended = projectile.target_id;
                    let pos = projectile.position;
                    let sneak_now = crate::game_logic::host_historic_bonus::logic_frame();
                    let shooter_obj = objects.get(&shooter);
                    let proj_owner = shooter_obj
                        .and_then(|s| s.owner_player_id)
                        .or(projectile.source_owner_player_id);
                    let proj_team = shooter_obj
                        .map(|s| s.team)
                        .unwrap_or(projectile.source_team);
                    // C++ Weapon.cpp:663-666 — never collide with the launcher's container.
                    let launcher_contained_by = shooter_obj.and_then(|s| s.contained_by);
                    // C++ Weapon.cpp:670-677 — Flame/ParticleBeam skip already-burned.
                    let skip_burned = matches!(
                        projectile.damage_type,
                        DamageType::Flame | DamageType::ParticleBeam
                    );
                    for (&oid, obj) in objects.iter() {
                        if oid == shooter || Some(oid) == intended {
                            continue;
                        }
                        if launcher_contained_by == Some(oid) {
                            continue;
                        }
                        if skip_burned && obj.has_object_status_bit("BURNED") {
                            continue;
                        }
                        if obj.get_sneaky_targeting_offset(sneak_now).is_some() {
                            continue;
                        }
                        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                            continue;
                        }
                        if obj.status.under_construction {
                            continue;
                        }
                        let same_controller =
                            crate::game_logic::weapon_bootstrap::projectile_structure_same_controller(
                                proj_owner,
                                obj.owner_player_id,
                                obj.team == proj_team,
                            );
                        if !crate::game_logic::weapon_bootstrap::projectile_collides_with_structure(
                            projectile.projectile_collides,
                            same_controller,
                        ) {
                            continue;
                        }
                        // C++ Weapon.cpp:679-699 — only skip KINDOF_FS_AIRFIELD
                        // when the intended victim reserved a parking space.
                        // Other buildings still detonate AA / airborne shots.
                        if obj.is_kind_of(KindOf::FSAirfield)
                            && obj.thing.template.parking_place.is_some()
                        {
                            if let Some(tid) = intended {
                                if let Some(t) = objects.get(&tid) {
                                    if t.producer_id == Some(oid) || t.contained_by == Some(oid) {
                                        continue;
                                    }
                                }
                            }
                        }
                        let radius = obj.selection_radius.max(8.0);
                        // Horizontal (XZ) distance — tall buildings block regardless of Y.
                        let op = obj.get_position();
                        let dx = pos.x - op.x;
                        let dz = pos.z - op.z;
                        if (dx * dx + dz * dz).sqrt() <= radius {
                            hit_structure = Some(oid);
                            break;
                        }
                    }
                }
                if let Some(sid) = hit_structure {
                    if let Some(flight) = projectile.flight.clone() {
                        if crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                            objects,
                            sid,
                            projectile.shooter_id,
                            &flight,
                        )
                        .is_some()
                        {
                            Self::play_garrison_hit_kill_fx(
                                objects,
                                sid,
                                flight.garrison_hit_kill_fx(),
                            );
                            projectiles_to_remove.push(proj_id);
                            continue;
                        }
                    }
                    let impact = projectile.position;
                    Self::maybe_record_historic_bonus(projectile, impact, objects);
                    if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                        damage_events.push(Self::splash_area_event(projectile, objects, impact));
                    } else if !projectile.no_damage {
                        damage_events.push(DamageEvent::Direct {
                            target_id: sid,
                            position: impact,
                            damage: projectile.damage,
                            damage_type: projectile.damage_type,
                            death_type: projectile.death_type,

                            shooter_id: projectile.shooter_id,
                        });
                    }
                    if !projectile.detonation_fx_name.is_empty()
                        || !projectile.detonation_ocl_name.is_empty()
                    {
                        self.impact_fx.push(ProjectileImpactFx {
                            position: impact,
                            shooter_id: projectile.shooter_id,
                            target_id: Some(sid),
                            detonation_fx_name: projectile.detonation_fx_name.clone(),
                            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                            source_team: projectile.source_team,
                            source_veterancy: projectile.source_veterancy,
                            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                            source_velocity: projectile.velocity,
                        });
                    }
                    Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                    projectiles_to_remove.push(proj_id);
                    continue;
                }

                // Check for hits
                if let Some(target_id) = projectile.target_id {
                    if let Some(target) = objects.get(&target_id) {
                        let distance = projectile.position.distance(target.get_position());
                        if distance <= 5.0 {
                            if let Some(flight) = projectile.flight.clone() {
                                if crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                                    objects,
                                    target_id,
                                    projectile.shooter_id,
                                    &flight,
                                )
                                .is_some()
                                {
                                    Self::play_garrison_hit_kill_fx(
                                        objects,
                                        target_id,
                                        flight.garrison_hit_kill_fx(),
                                    );
                                    projectiles_to_remove.push(proj_id);
                                    continue;
                                }
                            }
                            let impact = projectile.position;
                            Self::maybe_record_historic_bonus(projectile, impact, objects);
                            if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                                // C++ Weapon.cpp:1438: primary inside primaryRadius, else secondary.
                                damage_events
                                    .push(Self::splash_area_event(projectile, objects, impact));
                            } else if !projectile.no_damage {
                                damage_events.push(DamageEvent::Direct {
                                    target_id,
                                    position: impact,
                                    damage: projectile.damage,
                                    damage_type: projectile.damage_type,
                                    death_type: projectile.death_type,

                                    shooter_id: projectile.shooter_id,
                                });
                            }
                            if !projectile.detonation_fx_name.is_empty()
                                || !projectile.detonation_ocl_name.is_empty()
                            {
                                self.impact_fx.push(ProjectileImpactFx {
                                    position: impact,
                                    shooter_id: projectile.shooter_id,
                                    target_id: Some(target_id),
                                    detonation_fx_name: projectile.detonation_fx_name.clone(),
                                    detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                    source_team: projectile.source_team,
                                    source_veterancy: projectile.source_veterancy,
                                    source_orientation: projectile
                                        .velocity
                                        .z
                                        .atan2(projectile.velocity.x),
                                    source_velocity: projectile.velocity,
                                });
                            }
                            Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                            projectiles_to_remove.push(proj_id);
                        }
                    }
                } else {
                    // Check ground impact
                    let distance = projectile.position.distance(projectile.target_position);
                    if distance <= 2.0 {
                        let impact = projectile.target_position;
                        Self::maybe_record_historic_bonus(projectile, impact, objects);
                        if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                            damage_events
                                .push(Self::splash_area_event(projectile, objects, impact));
                        }
                        if !projectile.detonation_fx_name.is_empty()
                            || !projectile.detonation_ocl_name.is_empty()
                        {
                            self.impact_fx.push(ProjectileImpactFx {
                                position: impact,
                                shooter_id: projectile.shooter_id,
                                target_id: None,
                                detonation_fx_name: projectile.detonation_fx_name.clone(),
                                detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                source_team: projectile.source_team,
                                source_veterancy: projectile.source_veterancy,
                                source_orientation: projectile
                                    .velocity
                                    .z
                                    .atan2(projectile.velocity.x),
                                source_velocity: projectile.velocity,
                            });
                        }
                        Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                        projectiles_to_remove.push(proj_id);
                    }
                }
            }
        }

        self.apply_damage_events(&damage_events, objects, players);

        // Remove expired/hit projectiles.  Under coupled GameWorld flight
        // authority, publish an explicit inactive residual here: a later
        // active-only snapshot cannot otherwise tell the shadow that a host
        // KILL_SELF delay (or ordinary impact) has actually completed.
        for proj_id in &projectiles_to_remove {
            if self.projectiles.remove(proj_id).is_some()
                && crate::gameworld_shadow::gameworld_projectile_authority_live()
            {
                crate::game_logic::host_projectile_log::record_retired(proj_id.0);
            }
        }

        projectiles_to_remove
    }

    fn apply_damage_events(
        &mut self,
        damage_events: &[DamageEvent],
        objects: &mut HashMap<ObjectId, Object>,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
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
                    crate::game_logic::object::prime_live_damage_context(
                        objects.get(shooter_id),
                        None,
                        *damage_type,
                    );
                    if let Some(target) = objects.get_mut(target_id) {
                        let before = target.health.current;
                        let destroyed = target.take_damage_from_typed_death(
                            *damage,
                            Some(*shooter_id),
                            *damage_type,
                            *death_type,
                        );
                        let hp_lost = (before - target.health.current).max(0.0);
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
                    let splash_fx_source = objects.get(shooter_id).map(
                        crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source,
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
                    for (vid, obj) in objects.iter_mut() {
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
                                Some(map) => {
                                    crate::game_logic::GameLogic::object_relationship_from_owners(
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
                                crate::game_logic::host_transition_damage_fx::set_damage_fx_source(
                                    splash_fx_source.clone(),
                                );
                                let before = obj.health.current;
                                obj.take_damage_from_typed_death(
                                    area_damage,
                                    Some(*shooter_id),
                                    *damage_type,
                                    *death_type,
                                );
                                let hp_lost = (before - obj.health.current).max(0.0);
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
        objects: &mut HashMap<ObjectId, Object>,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
    ) {
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
            .or_else(|| objects.get(&p.shooter_id).and_then(|o| o.owner_player_id));
        proj.source_team_instance_name = objects
            .get(&p.shooter_id)
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

        Self::maybe_record_historic_bonus(&proj, shot.damage_pos, objects);
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
        self.apply_damage_events(&events, objects, players);
    }

    /// Check if projectile collides with something
    fn check_projectile_collision(
        &self,
        projectile: &Projectile,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<ProjectileHit> {
        // Check target collision
        if let Some(target_id) = projectile.target_id {
            if let Some(target) = objects.get(&target_id) {
                let now = crate::game_logic::host_historic_bonus::logic_frame();
                if target.get_sneaky_targeting_offset(now).is_some() {
                    return None;
                }
                let distance = projectile.position.distance(target.get_position());
                if distance <= 5.0 {
                    return Some(ProjectileHit::Direct {
                        target_id,
                        position: projectile.position,
                        damage: projectile.damage,
                        damage_type: projectile.damage_type,
                        death_type: projectile.death_type,
                    });
                }
            }
        }

        // Check ground collision
        let distance = projectile.position.distance(projectile.target_position);
        if distance <= 2.0 && projectile.explosion_radius > 0.0 {
            return Some(ProjectileHit::Area {
                position: projectile.target_position,
                damage: projectile.damage,
                damage_type: projectile.damage_type,
                death_type: projectile.death_type,

                radius: projectile.explosion_radius,
                shooter_id: projectile.shooter_id,
            });
        }

        None
    }

    /// Get all active projectiles
    pub fn get_projectiles(&self) -> &HashMap<ObjectId, Projectile> {
        &self.projectiles
    }

    /// Clear all projectiles
    pub fn clear(&mut self) {
        self.projectiles.clear();
    }
}

fn host_projectile_is_small_missile(projectile: &Projectile) -> bool {
    if projectile.is_small_missile {
        return true;
    }
    if crate::game_logic::host_car_bomb::object_definition_has_kind(
        &projectile.projectile_object_name,
        "SMALL_MISSILE",
    ) {
        return true;
    }
    if crate::game_logic::host_car_bomb::object_definition_has_kind(
        &projectile.projectile_object_name,
        "BALLISTIC_MISSILE",
    ) {
        return false;
    }
    matches!(
        projectile.damage_type,
        DamageType::InfantryMissile
            | DamageType::JetMissiles
            | DamageType::StealthJetMissiles
            | DamageType::SubdualMissile
    ) || matches!(
        projectile.projectile_lifecycle,
        Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
    )
}

/// C++ Weapon.cpp:1144-1155 report at launch; MissileAIUpdate decoy when the timer expires.
fn apply_countermeasure_report_and_decoy(
    projectile: &mut Projectile,
    proj_id: ObjectId,
    objects: &HashMap<ObjectId, Object>,
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
        if host_projectile_is_small_missile(projectile) {
            if let Some(tid) = projectile.target_id {
                if let Some(target) = objects.get(&tid) {
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
            let aircraft_xz = objects.get(&victim_id).map(|o| {
                let p = o.get_position();
                (p.x, p.z)
            });
            if let Some(axz) = aircraft_xz {
                let flare_xz: Vec<(ObjectId, f32, f32)> = objects
                    .iter()
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
                    if let Some(flare) = objects.get(&fid) {
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
    let geom = &obj.thing.template.geometry_info;
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
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store(|store| {
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
