use super::*;

impl Object {
    /// C++ StructureCollapseUpdate::onDie / beginStructureCollapse residual.
    pub fn begin_structure_collapse(&mut self, current_frame: u32) -> bool {
        if !self.is_kind_of(crate::game_logic::KindOf::Structure) {
            return false;
        }
        if !crate::game_logic::host_structure_collapse::is_structure_collapse_candidate(
            &self.template_name,
            true,
        ) {
            return false;
        }
        if self
            .structure_collapse_data
            .as_ref()
            .map(|d| !d.is_standing())
            .unwrap_or(false)
        {
            return true;
        }
        let mut data =
            crate::game_logic::host_structure_collapse::HostStructureCollapseData::default();
        let radius = self.selection_radius.max(10.0);
        data.building_height = (self.health.maximum.max(100.0) * 0.12)
            .clamp(15.0, 60.0)
            .max(radius * 0.8);
        if let Some(ini) = crate::game_logic::host_structure_collapse::collapse_ini_for_template(
            &self.template_name,
        ) {
            data.bind_ini(&ini);
        }
        // Mid delay residual (average of 15–30).
        data.begin(current_frame, 22);
        self.structure_collapse_data = Some(data);
        if let Some(bfx) = self.bone_fx_damage.as_mut() {
            bfx.stop_all_bone_fx();
        }
        self.selected = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        // C++ beginStructureCollapse: doPhaseStuff(SCPHASE_INITIAL) → FXList::doFXPos.
        self.dispatch_pending_collapse_fx();
        true
    }

    /// C++ StructureCollapseUpdate::update residual. True when collapse completes.
    pub fn tick_structure_collapse(&mut self, current_frame: u32) -> bool {
        let Some(sc) = self.structure_collapse_data.as_mut() else {
            return false;
        };
        let done = sc.tick(current_frame);
        self.dispatch_pending_collapse_fx();
        if !done {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    /// Drain INITIAL/BURST/DELAY/FINAL when a dual-peel owns sink motion.
    pub fn poll_structure_collapse_phase_fx(&mut self, current_frame: u32) {
        let Some(sc) = self.structure_collapse_data.as_mut() else {
            return;
        };
        sc.poll_phase_fx(current_frame);
        self.dispatch_pending_collapse_fx();
    }

    /// C++ `doPhaseStuff` → `FXList::doFXPos` at the building position.
    fn dispatch_pending_collapse_fx(&mut self) {
        let pos = self.get_position();
        let Some(sc) = self.structure_collapse_data.as_mut() else {
            return;
        };
        for fx in sc.take_pending_phase_fx() {
            if !fx.is_empty() && !fx.eq_ignore_ascii_case("None") {
                let _ = crate::game_logic::dispatch_fx_list_at_pos(&fx, pos);
            }
        }
    }

    /// Presentation vertical offset from structure collapse residual.
    pub fn presentation_collapse_height_offset(&self) -> f32 {
        self.structure_collapse_data
            .as_ref()
            .map(|d| d.collapse_height_offset())
            .unwrap_or(0.0)
    }

    /// Presentation shudder residual from structure collapse.
    pub fn presentation_collapse_shudder(&self) -> (f32, f32) {
        self.structure_collapse_data
            .as_ref()
            .map(|d| (d.shudder_x, d.shudder_z))
            .unwrap_or((0.0, 0.0))
    }

    /// C++ StructureToppleUpdate::onDie / beginStructureTopple residual.
    /// Call when a structure reaches lethal damage instead of instant destroy.
    /// Returns true if structure topple was started (caller should not destroy yet).
    pub fn begin_structure_topple(
        &mut self,
        current_frame: u32,
        attacker_pos: Option<(f32, f32)>,
    ) -> bool {
        if !self.is_kind_of(crate::game_logic::KindOf::Structure) {
            return false;
        }
        if !crate::game_logic::host_structure_topple::is_structure_topple_candidate(
            &self.template_name,
            true,
        ) {
            return false;
        }
        if self
            .structure_topple_data
            .as_ref()
            .map(|d| !d.is_standing())
            .unwrap_or(false)
        {
            return true; // already toppling
        }
        let pos = self.get_position();
        let (mut dx, mut dz) = match attacker_pos {
            Some((ax, az)) => (pos.x - ax, pos.z - az),
            None => (1.0, 0.0),
        };
        // C++ StructureToppleUpdate/ToppleUpdate::adjustToppleDirection.
        if !self.name.is_empty() {
            let leftover = gamelogic::scripting::engine::with_script_engine_ref(|engine| {
                engine.topple_direction_for_name(&self.name)
            })
            .flatten();
            let mapped = leftover
                .map(|d| (d.x, d.y))
                .or_else(|| gamelogic::scripting::host_script_topple_direction_for(&self.name));
            if let Some((sx, sy)) = mapped {
                if sx * sx + sy * sy > 1e-12 {
                    dx = sx;
                    dz = sy;
                }
            }
        }
        let mut data = crate::game_logic::host_structure_topple::HostStructureToppleData::default();
        let geom = &self.thing.template.geometry_info;
        data.bind_geometry(
            geom.max_height_above_position(),
            geom.major_radius,
            geom.minor_radius,
            self.get_orientation(),
        );
        data.bind_world_pos(pos.x, pos.y, pos.z);
        if let Some(peel) =
            crate::game_logic::host_structure_topple::leftover_structure_topple_module_peel(
                &self.template_name,
            )
        {
            if !peel.weapon.is_empty() {
                data.bind_crushing_weapon(&peel.weapon);
            } else {
                data.bind_crushing_weapon(
                    crate::game_logic::host_structure_topple::STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME,
                );
            }
            if !peel.fx.is_empty() {
                data.crushing_fx = peel.fx.clone();
            }
            data.bind_topple_fx(&peel);
        } else {
            data.bind_crushing_weapon(
                crate::game_logic::host_structure_topple::STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME,
            );
        }
        data.begin(current_frame, dx, dz, 0);
        self.structure_topple_data = Some(data);
        self.dispatch_pending_structure_topple_fx();
        if let Some(bfx) = self.bone_fx_damage.as_mut() {
            bfx.stop_all_bone_fx();
        }
        // C++ marks AI dead and deselects while building is still "alive" for fall.
        self.selected = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        // Keep a sliver of HP so generic destroy passes leave it alone until done.
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        true
    }

    /// Attach FireWeaponWhenDamaged residual when template peels match.

    pub fn ensure_height_die(&mut self, current_frame: u32) {
        if self.height_die.is_some() {
            return;
        }
        if let Some((h, desc, delay_ms)) =
            crate::game_logic::host_height_die::height_die_config_for_template(&self.template_name)
        {
            let delay_f = ((delay_ms as f32) * 30.0 / 1000.0).round() as u32;
            self.height_die = Some(
                crate::game_logic::host_height_die::HostHeightDieData::with_target(
                    h,
                    desc,
                    current_frame.saturating_add(delay_f),
                ),
            );
        }
    }

    /// C++ HeightDieUpdate residual. True when should die from altitude.
    /// C++ FuelAir gas SlowDeathBehavior residual install.
    pub fn ensure_fuel_air_gas_slow_death(&mut self, current_frame: u32) {
        if self.fuel_air_gas_slow_death.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasSlowDeathData::for_template(
                &self.template_name,
                current_frame,
            )
        {
            self.fuel_air_gas_slow_death = Some(data);
            // Retail HeightDie TargetHeight 15 on gas.
            if self.height_die.is_none() {
                self.height_die = Some(
                    crate::game_logic::host_height_die::HostHeightDieData::with_target(
                        crate::game_logic::host_fuel_air_gas_slow_death::FUEL_AIR_GAS_HEIGHT_DIE,
                        false,
                        current_frame,
                    ),
                );
            }
        }
    }

    /// C++ NeutronMissileUpdate residual install when target known.
    pub fn ensure_neutron_missile_update(
        &mut self,
        target: glam::Vec3,
        launcher: Option<crate::game_logic::ObjectId>,
        now: u32,
    ) {
        if self.neutron_missile_update.is_some() {
            return;
        }
        let launch = self.get_position();
        if let Some(data) =
            crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateData::for_template(
                &self.template_name,
                launch,
                target,
                launcher.map(|id| id.0),
                now,
            )
        {
            self.neutron_missile_update = Some(data);
        }
    }

    pub fn tick_height_die(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        self.ensure_height_die(current_frame);
        let pos = self.get_position();
        let hat = pos.y - terrain_height;
        let contained = self.contained_by.is_some();
        let Some(hd) = self.height_die.as_mut() else {
            return false;
        };
        if hd.tick(current_frame, hat, contained) {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ KeepObjectDie residual: leave rubble instead of DestroyDie remove.
    pub fn begin_keep_object_die(&mut self, current_frame: u32) -> bool {
        let is_struct = self.is_kind_of(crate::game_logic::KindOf::Structure);
        if !crate::game_logic::host_keep_object_die::wants_keep_object_die(
            &self.template_name,
            is_struct,
        ) {
            return false;
        }
        if self.status.keep_as_rubble {
            return true;
        }
        let mut data = crate::game_logic::host_keep_object_die::HostKeepObjectDieData::default();
        data.mark_rubble(current_frame);
        self.keep_object_die = Some(data);
        self.health.current = 0.0;
        self.status.effectively_dead = true;
        self.status.keep_as_rubble = true;
        // C++ ActiveBody::setCorrectDamageState rubble: collapse Z +
        // OBJECT_STATUS_NO_COLLISIONS (ActiveBody.cpp:190-208).
        self.apply_structure_rubble_collision_effects();
        // Not destroyed: remains in world. Unselectable rubble husk.
        self.status.destroyed = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        self.refresh_model_condition_bits();
        true
    }

    /// C++ `ActiveBody::setCorrectDamageState` structure-rubble side effects.
    pub fn apply_structure_rubble_collision_effects(&mut self) {
        if !self.is_kind_of(crate::game_logic::KindOf::Structure) {
            return;
        }
        let rubble_h = if self.thing.template.structure_rubble_height > 0 {
            self.thing.template.structure_rubble_height as f32
        } else {
            host_structure_rubble_height(&self.template_name)
        };
        if self.thing.template.geometry_info.authored {
            match self.thing.template.geometry_info.geom_type {
                crate::game_logic::HostGeometryType::Sphere => {
                    self.thing.template.geometry_info.major_radius = rubble_h;
                }
                crate::game_logic::HostGeometryType::Box
                | crate::game_logic::HostGeometryType::Cylinder => {
                    self.thing.template.geometry_info.height = rubble_h;
                }
            }
        }
        let g = &mut self.thing.geometry;
        g.bounds_max.y = g.bounds_min.y + rubble_h;
        self.set_status_no_collisions(true);
    }

    /// C++ SlowDeathBehavior::beginSlowDeath residual.
    /// Returns true if slow death started (caller should defer destroy).
    /// Starts only when Object INI authors `SlowDeathBehavior`.
    pub fn begin_slow_death(&mut self, current_frame: u32) -> bool {
        let Some(ini) =
            crate::game_logic::host_slow_death::slow_death_ini_for_template(&self.template_name)
        else {
            return self
                .slow_death
                .as_ref()
                .map(|s| s.is_active())
                .unwrap_or(false);
        };
        self.begin_slow_death_from_ini(current_frame, &ini)
    }

    /// C++ SlowDeathBehavior::beginSlowDeath with authored module data.
    pub fn begin_slow_death_from_ini(
        &mut self,
        current_frame: u32,
        ini: &crate::game_logic::host_slow_death::HostSlowDeathIni,
    ) -> bool {
        if self
            .slow_death
            .as_ref()
            .map(|s| s.is_active() || s.is_done())
            .unwrap_or(false)
        {
            return self
                .slow_death
                .as_ref()
                .map(|s| s.is_active())
                .unwrap_or(false);
        }
        let mut sd =
            crate::game_logic::host_slow_death::HostSlowDeathData::from_ini(current_frame, ini);
        if ini.fling_force > 0.0 {
            let ang = (self.id.0 as f32) * 0.618_033_988;
            sd.apply_fling_angle(ang);
        }
        if let Some((fx, fy, fz)) = sd.take_fling_impulse() {
            self.movement.velocity.x += fx;
            self.movement.velocity.y += fy;
            self.movement.velocity.z += fz;
            // C++ SlowDeathBehavior.cpp:282 setExtraFriction(-3 * SECONDS_PER_LOGICFRAME).
            self.set_extra_friction(-3.0 * (1.0 / 30.0));
            self.allow_to_fall = true;
            self.shock_allow_bounce = true;
        }
        self.apply_slow_death_phase_fx(&mut sd);
        self.slow_death = Some(sd);
        // Keep a sliver of HP bookkeeping like structure topple residual.
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        // Mark "dead" for AI but not removed yet.
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        self.selected = false;
        self.status.selected = false;
        self.status.destroyed = false;
        true
    }

    /// C++ SlowDeathBehavior::update residual. True when ready to destroyObject.
    pub fn tick_slow_death(&mut self, current_frame: u32) -> bool {
        let Some(sd) = self.slow_death.as_mut() else {
            return false;
        };
        let done = sd.tick(current_frame);
        let fx = sd.take_pending_phase_fx();
        if let Some(name) = fx.into_iter().last() {
            self.pending_death_fx = Some(name);
        }
        if !done {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        true
    }

    fn apply_slow_death_phase_fx(
        &mut self,
        sd: &mut crate::game_logic::host_slow_death::HostSlowDeathData,
    ) {
        if let Some(name) = sd.take_pending_phase_fx().into_iter().last() {
            self.pending_death_fx = Some(name);
        }
    }

    /// Drain INITIAL/MIDPOINT/FINAL when a dual-peel owns sink motion.
    pub fn poll_slow_death_phase_fx(&mut self, current_frame: u32) {
        let Some(sd) = self.slow_death.as_mut() else {
            return;
        };
        sd.poll_phase_fx(current_frame);
        if let Some(name) = sd.take_pending_phase_fx().into_iter().last() {
            self.pending_death_fx = Some(name);
        }
    }

    pub fn presentation_slow_death_sink_offset(&self) -> f32 {
        self.slow_death
            .as_ref()
            .map(|s| s.sink_offset)
            .unwrap_or(0.0)
    }

    pub fn ensure_create_object_die(&mut self) {
        if self.create_object_die.is_some() {
            return;
        }
        if let Some(cfg) =
            crate::game_logic::host_create_object_die::create_object_die_config_for_template(
                &self.template_name,
            )
        {
            self.create_object_die = Some(cfg);
        }
    }

    pub fn ensure_lifetime_update(&mut self, current_frame: u32) {
        if self.lifetime_update.is_some() {
            return;
        }
        if let Some((min_ms, max_ms)) =
            crate::game_logic::host_lifetime_update::lifetime_msec_range_for_template(
                &self.template_name,
            )
        {
            let is_hulk = self.template_name.to_ascii_lowercase().contains("hulk");
            self.lifetime_update = Some(
                crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_ini_range(
                    current_frame,
                    min_ms,
                    max_ms,
                    self.id.0,
                    is_hulk,
                ),
            );
        }
    }

    /// C++ CreateObjectDie::onDie residual — queues spawn templates.
    pub fn fire_create_object_die(&mut self) {
        self.ensure_create_object_die();
        let Some(cod) = self.create_object_die.as_mut() else {
            return;
        };
        let transfer = cod.transfer_previous_health;
        if let Some(spawns) = cod.on_die() {
            self.pending_create_object_die_spawns = spawns;
            if transfer {
                let max_h = self.health.maximum.max(self.max_health).max(1.0);
                // C++ getPreviousHealth: health before last damage, not current (often 0).
                let prev = if self.previous_health > 0.0 {
                    self.previous_health
                } else if self.health.current > 0.01 {
                    self.health.current
                } else {
                    max_h
                };
                self.create_object_die_transfer_damage = (max_h - prev).max(0.0);
                self.create_object_die_transfer_subdual = self.subdual_damage.max(0.0);
                self.create_object_die_transfer_source = self.last_damage_source;
            }
        }
    }

    pub fn take_pending_create_object_die_spawns(
        &mut self,
    ) -> (
        Vec<String>,
        f32,
        bool,
        f32,
        Option<crate::game_logic::ObjectId>,
    ) {
        let spawns = std::mem::take(&mut self.pending_create_object_die_spawns);
        let dmg = self.create_object_die_transfer_damage;
        self.create_object_die_transfer_damage = 0.0;
        let subdual = self.create_object_die_transfer_subdual;
        self.create_object_die_transfer_subdual = 0.0;
        let source = self.create_object_die_transfer_source.take();
        let transfer = self
            .create_object_die
            .as_ref()
            .map(|c| c.transfer_previous_health)
            .unwrap_or(false);
        (spawns, dmg, transfer, subdual, source)
    }

    /// C++ LifetimeUpdate residual. True when object should die this frame.
    pub fn tick_lifetime_update(&mut self, current_frame: u32) -> bool {
        self.ensure_lifetime_update(current_frame);
        self.lifetime_update
            .as_ref()
            .map(|l| l.tick(current_frame))
            .unwrap_or(false)
    }

    pub fn ensure_transition_damage_fx(&mut self) {
        if self.transition_damage_fx.is_some() {
            return;
        }
        let is_structure = self.is_kind_of(crate::game_logic::KindOf::Structure);
        let is_vehicle = self.is_kind_of(crate::game_logic::KindOf::Vehicle);
        if let Some(cfg) =
            crate::game_logic::host_transition_damage_fx::transition_damage_fx_config_for_template(
                &self.template_name,
                is_structure,
                is_vehicle,
            )
        {
            self.transition_damage_fx = Some(cfg);
        }
    }

    pub fn ensure_fx_list_die(&mut self) {
        if self.fx_list_die.is_some() {
            return;
        }
        if let Some(cfg) = crate::game_logic::host_fx_list_die::fx_list_die_config_for_template(
            &self.template_name,
        ) {
            self.fx_list_die = Some(cfg);
        }
    }

    pub fn take_pending_transition_damage_fx(
        &mut self,
    ) -> Vec<crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent> {
        std::mem::take(&mut self.pending_transition_damage_fx)
    }

    pub fn take_pending_death_fx_audio(&mut self) -> (Option<String>, Option<String>) {
        (
            self.pending_death_fx.take(),
            self.pending_death_audio.take(),
        )
    }

    pub fn take_pending_death_audio_stop(&mut self) -> bool {
        std::mem::take(&mut self.pending_death_audio_stop)
    }

    pub fn fire_fx_list_die(&mut self) {
        self.ensure_fx_list_die();
        let upgrades: Vec<String> = self.applied_upgrades.iter().cloned().collect();
        let death_type = self.status.death_type;
        // C++ FXListDie::onDie → doFXObj when OrientToObject, else doFXPos.
        let killer = self.last_damage_source.map(|id| id.0);
        let pos = self.get_position();
        let Some(fx) = self.fx_list_die.as_mut() else {
            return;
        };
        let leftover_vet = crate::game_logic::host_fx_list_die::leftover_veterancy_from_host(
            self.experience.level,
        );
        let leftover_status = self.object_status_bits;
        let hits =
            fx.collect_applicable_mux_hits(&upgrades, death_type, leftover_vet, leftover_status);
        for (i, hit) in hits.into_iter().enumerate() {
            if !hit.orient_to_object {
                if let Some(name) = hit.death_fx {
                    let _ = crate::game_logic::dispatch_fx_list_at_pos(&name, pos);
                }
                if self.pending_death_audio.is_none() {
                    self.pending_death_audio = hit.death_audio;
                }
                continue;
            }
            if i == 0 {
                if self.pending_death_fx.is_none() {
                    self.pending_death_fx = hit.death_fx;
                }
                if self.pending_death_audio.is_none() {
                    self.pending_death_audio = hit.death_audio;
                }
            } else if let Some(name) = hit.death_fx {
                let _ = crate::game_logic::dispatch_fx_list_at_object(&name, self.id.0, killer);
                if self.pending_death_audio.is_none() {
                    self.pending_death_audio = hit.death_audio;
                }
            }
        }
    }

    pub fn ensure_crush_die(&mut self) {
        if self.crush_die.is_some() {
            return;
        }
        if let Some(cfg) =
            crate::game_logic::host_crush_die::crush_die_config_for_template(&self.template_name)
        {
            self.crush_die = Some(cfg);
        }
    }

    /// C++ CrushDie::onDie — location check, body flags, model bits, sound.
    pub fn fire_crush_die(&mut self) {
        self.fire_crush_die_from_crusher(None);
    }

    /// `crusher_xz` is the dealer host XZ (C++ `damageDealer` position).
    /// Missing dealer defaults to `TOTAL_CRUSH` when neither end is stamped.
    pub fn fire_crush_die_from_crusher(&mut self, crusher_xz: Option<(f32, f32)>) {
        if !matches!(
            self.status.death_type,
            crate::game_logic::host_usa_pilot::HostDeathType::Crushed
        ) {
            return;
        }
        self.ensure_crush_die();
        let kind = if let Some(crusher_xz) = crusher_xz {
            let victim = self.get_position();
            let geom = &self.thing.template.geometry_info;
            let major = if geom.authored {
                geom.major_radius
            } else {
                self.selection_radius.max(1.0)
            };
            match crate::game_logic::host_crush_die::crush_location_check(
                crusher_xz,
                (victim.x, victim.z),
                self.unit_direction_xz(),
                major,
                self.front_crushed,
                self.back_crushed,
            ) {
                Some(kind) => kind,
                None => return,
            }
        } else if !self.front_crushed && !self.back_crushed {
            crate::game_logic::host_crush_die::HostCrushKind::Total
        } else {
            crate::game_logic::host_crush_die::crush_kind_from_flags(
                self.front_crushed,
                self.back_crushed,
            )
        };
        let (front, back) = crate::game_logic::host_crush_die::flags_from_crush_kind(kind);
        self.front_crushed = front;
        self.back_crushed = back;
        self.apply_crush_die_model_conditions();
        let seed = self.id.0;
        let Some(crush) = self.crush_die.as_mut() else {
            return;
        };
        if let Some(sound) = crush.on_die(kind, seed) {
            if self.pending_death_audio.is_none() {
                self.pending_death_audio = Some(sound);
            }
        }
    }

    /// C++ InstantDeathBehavior::onDie residual — queues FX/OCL/Weapon.
    pub fn fire_instant_death(&mut self) -> bool {
        let Some(ini) = crate::game_logic::host_instant_death::first_applicable_instant_death(
            &self.template_name,
            self.status.under_construction,
            self.status.death_type,
        ) else {
            return false;
        };
        let burst = ini.pick(self.id.0);
        if let Some(fx) = burst.fx {
            if self.pending_death_fx.is_none() {
                self.pending_death_fx = Some(fx);
            }
        }
        if let Some(ocl) = burst.ocl {
            let templates =
                crate::game_logic::host_create_object_die::peel_ocl_spawn_templates(&ocl);
            if !templates.is_empty() {
                self.pending_create_object_die_spawns = templates;
            } else {
                self.pending_create_object_die_spawns = vec![ocl];
            }
        }
        self.pending_instant_death_weapon = burst.weapon;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    pub fn ensure_fire_weapon_when_damaged(&mut self) {
        if self.fire_weapon_when_damaged.is_some() {
            return;
        }
        if let Some(cfg) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_config_for_template(
                &self.template_name,
            )
        {
            self.fire_weapon_when_damaged = Some(cfg);
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior::onDamage residual.
    /// Returns weapon name to force-fire at self position.
    pub fn take_pending_fire_when_damaged_weapon(&mut self) -> Option<String> {
        self.pending_fire_when_damaged_weapon.take()
    }

    pub fn on_fire_weapon_when_damaged(
        &mut self,
        actual_damage: f32,
        current_frame: u32,
        damage_type_ordinal: u32,
    ) -> Option<String> {
        self.ensure_fire_weapon_when_damaged();
        let Some(fw) = self.fire_weapon_when_damaged.as_mut() else {
            return None;
        };
        fw.on_damage(
            actual_damage,
            self.health.current,
            self.health.maximum.max(self.max_health).max(1.0),
            current_frame,
            damage_type_ordinal,
        )
    }

    /// C++ FireWeaponWhenDamagedBehavior continuous update residual.
    pub fn tick_fire_weapon_when_damaged_continuous(
        &mut self,
        current_frame: u32,
    ) -> Option<String> {
        self.ensure_fire_weapon_when_damaged();
        let Some(fw) = self.fire_weapon_when_damaged.as_mut() else {
            return None;
        };
        fw.tick_continuous(
            self.health.current,
            self.health.maximum.max(self.max_health).max(1.0),
            current_frame,
        )
    }

    /// Drain C++ applyCrushingDamage residual samples for this frame.
    pub fn take_structure_topple_crush_samples(
        &mut self,
    ) -> Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample> {
        let pos = self.get_position();
        let Some(st) = self.structure_topple_data.as_mut() else {
            return Vec::new();
        };
        if !(st.is_active()
            || matches!(
                st.state,
                crate::game_logic::host_structure_topple::HostStructureToppleState::Done
            ))
        {
            return Vec::new();
        }
        st.take_crush_sweep_samples(pos.x, pos.z)
    }

    /// C++ StructureToppleUpdate::update residual. True when fall completes.
    pub fn tick_structure_topple(&mut self, current_frame: u32) -> bool {
        let finished = {
            let Some(st) = self.structure_topple_data.as_mut() else {
                return false;
            };
            st.tick(current_frame)
        };
        self.dispatch_pending_structure_topple_fx();
        if !finished {
            return false;
        }
        // doToppleDoneStuff residual: finalize death.
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    fn dispatch_pending_structure_topple_fx(&mut self) {
        let pos = self.get_position();
        let ori = self.get_orientation();
        let player = self.owner_player_id.map(|p| p as i32).unwrap_or(-1);
        let id = self.id.0;
        if let Some(st) = self.structure_topple_data.as_mut() {
            st.dispatch_pending_fx(id, pos, ori, player);
        }
    }

    /// Drain leftover Start/Delay/Done/AngleFX when a dual-peel owns fall motion.
    pub fn poll_structure_topple_fx(&mut self, current_frame: u32) {
        if let Some(st) = self.structure_topple_data.as_mut() {
            st.poll_fx(current_frame);
        }
        self.dispatch_pending_structure_topple_fx();
    }

    /// Combined presentation lean (tree topple or structure topple).
    pub fn presentation_topple_lean_radians(&self) -> f32 {
        if let Some(st) = self.structure_topple_data.as_ref() {
            if st.is_active()
                || matches!(
                    st.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            {
                return st.lean_radians;
            }
        }
        self.topple_data
            .as_ref()
            .map(|t| t.lean_radians)
            .unwrap_or(0.0)
    }

    /// Attach residual ToppleUpdate when template is topple-capable.
    pub fn ensure_topple_data(&mut self) {
        if self.topple_data.is_none()
            && crate::game_logic::host_topple::is_topple_capable_template(&self.template_name)
        {
            let mut td = crate::game_logic::host_topple::HostToppleData::default();
            td.bind_authored_for_template(&self.template_name);
            self.topple_data = Some(td);
        }
        if let Some(td) = self.topple_data.as_mut() {
            td.bind_authored_for_template(&self.template_name);
        }
    }

    pub fn presentation_topple_dir(&self) -> (f32, f32) {
        self.topple_data
            .as_ref()
            .map(|t| (t.dir_x, t.dir_y))
            .unwrap_or((1.0, 0.0))
    }

    pub fn presentation_shadows_enabled(&self) -> bool {
        // C++ W3DModelDraw::allocateShadows / setModelState: Shadow == NONE allocates nothing.
        let bits = crate::game_logic::host_battlemaster::leftover_template_shadow_type(
            &self.template_name,
            self.thing.template.shadow_type,
        );
        if bits == 0 {
            return false;
        }
        let topple = self
            .topple_data
            .as_ref()
            .map(|t| t.shadows_enabled)
            .unwrap_or(true);
        if !topple {
            return false;
        }
        // C++ Drawable::draw: setShadowsEnabled(look != STEALTHLOOK_VISIBLE_DETECTED)
        // only while the bound object is not effectively dead.
        if self.is_alive()
            && crate::game_logic::host_upgrades::HostCamoStealthLook::from_u8(
                self.camo_stealth_look,
            ) == crate::game_logic::host_upgrades::HostCamoStealthLook::VisibleDetected
        {
            return false;
        }
        true
    }

    /// C++ Object::topple residual.
    /// Returns true if the object should be destroyed immediately (start-topple kill).
    pub fn apply_topple(
        &mut self,
        dir_x: f32,
        dir_y: f32,
        topple_speed: f32,
        options: u32,
    ) -> bool {
        if self.status.destroyed || !self.is_alive() {
            return false;
        }
        self.ensure_topple_data();
        let burned = {
            use crate::game_logic::host_enum_table_residual::burned_model_bit;
            (self.model_condition_bits & (1u128 << burned_model_bit())) != 0
                || self.has_object_status_bit("BURNED")
        };
        let pos = self.get_position();
        let kill_now = {
            let Some(td) = self.topple_data.as_mut() else {
                return false;
            };
            if !td.is_able_to_be_toppled() {
                return false;
            }
            td.burned_at_topple = burned;
            td.apply_toppling_force(dir_x, dir_y, topple_speed, options)
        };
        self.dispatch_pending_topple_fx(pos);
        if kill_now {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        }
        kill_now
    }

    fn dispatch_pending_topple_fx(&mut self, _pos: glam::Vec3) {
        let (topple, bounce) = {
            let Some(td) = self.topple_data.as_mut() else {
                return;
            };
            (td.take_pending_topple_fx(), td.take_pending_bounce_fx())
        };
        // C++ ToppleUpdate.cpp:189/324 FXList::doFXObj — tree transform,
        // object-shroud, Sound player index.
        for fx in [topple, bounce].into_iter().flatten() {
            crate::game_logic::publish_host_fx_object(
                self.id.0,
                self.get_position(),
                self.get_orientation(),
                self.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            );
            let _ = crate::game_logic::dispatch_fx_list_at_object(&fx, self.id.0, None);
        }
    }

    /// C++ ToppleUpdate::update residual. Returns true if death-by-topple this frame.
    pub fn tick_topple(&mut self) -> bool {
        let Some(td) = self.topple_data.as_mut() else {
            return false;
        };
        let died = td.tick();
        let pos = self.get_position();
        self.dispatch_pending_topple_fx(pos);
        if !died {
            return false;
        }
        // deathByToppling: UNRESISTABLE + DEATH_TOPPLED
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    /// C++ ToppleUpdate::onCollide residual when other crushes this prop.
    pub fn try_topple_from_crusher(
        &mut self,
        crusher_level: u8,
        from_x: f32,
        from_z: f32,
        speed: f32,
    ) -> bool {
        if !crate::game_logic::host_topple::crusher_can_topple(crusher_level) {
            return false;
        }
        self.ensure_topple_data();
        let Some(td) = self.topple_data.as_ref() else {
            return false;
        };
        if !td.is_able_to_be_toppled() {
            return false;
        }
        let pos = self.get_position();
        let dx = pos.x - from_x;
        let dz = pos.z - from_z;
        self.apply_topple(
            dx,
            dz,
            speed.max(1.0),
            crate::game_logic::host_topple::TOPPLE_OPTIONS_NONE,
        )
    }
}

/// C++ `ThingTemplate::getStructureRubbleHeight` then
/// `TheGlobalData->m_defaultStructureRubbleHeight` (ActiveBody.cpp:191-194).
fn host_structure_rubble_height(template_name: &str) -> f32 {
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                if let Some(h) = tmpl.structure_rubble_height() {
                    if h > 0 {
                        return h as f32;
                    }
                }
            }
        }
    }
    game_engine::common::global_data::read_safe()
        .ok()
        .map(|g| g.default_structure_rubble_height)
        .filter(|h| *h > 0.0)
        .unwrap_or(
            crate::game_logic::host_gamedata_lobby_residual::DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL,
        )
}
