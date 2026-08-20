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
        // Mid delay residual (average of 15–30).
        data.begin(current_frame, 22);
        self.structure_collapse_data = Some(data);
        self.selected = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        true
    }

    /// C++ StructureCollapseUpdate::update residual. True when collapse completes.
    pub fn tick_structure_collapse(&mut self, current_frame: u32) -> bool {
        let Some(sc) = self.structure_collapse_data.as_mut() else {
            return false;
        };
        if !sc.tick(current_frame) {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
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
        let (dx, dz) = match attacker_pos {
            Some((ax, az)) => (pos.x - ax, pos.z - az),
            None => (1.0, 0.0),
        };
        let mut data = crate::game_logic::host_structure_topple::HostStructureToppleData::default();
        let radius = self.selection_radius.max(10.0);
        data.facing_width = radius;
        data.building_height = (self.health.maximum.max(100.0) * 0.15).clamp(20.0, 80.0);
        data.begin(current_frame, dx, dz, 0);
        self.structure_topple_data = Some(data);
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
        // Not destroyed: remains in world. Unselectable rubble husk.
        self.status.destroyed = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        self.refresh_model_condition_bits();
        true
    }

    /// C++ SlowDeathBehavior::beginSlowDeath residual.
    /// Returns true if slow death started (caller should defer destroy).
    pub fn begin_slow_death(&mut self, current_frame: u32) -> bool {
        use crate::game_logic::host_slow_death::wants_slow_death;
        let is_inf = self.is_kind_of(crate::game_logic::KindOf::Infantry);
        let is_veh = self.is_kind_of(crate::game_logic::KindOf::Vehicle);
        if !wants_slow_death(&self.template_name, is_inf, is_veh) {
            return false;
        }
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
        let mut sd = crate::game_logic::host_slow_death::HostSlowDeathData::default();
        let ok = if is_inf {
            sd.begin_infantry(current_frame)
        } else {
            sd.begin_vehicle(current_frame)
        };
        if !ok {
            return false;
        }
        // sd may be replaced by fling residual below for infantry.
        // Optional fling residual (exploded infantry peel).
        if is_inf {
            // Deterministic angle from object id.
            let ang = (self.id.0 as f32) * 0.618_033_988;
            let mut fling_sd =
                crate::game_logic::host_slow_death::HostSlowDeathData::infantry_fling_residual(
                    current_frame,
                    40.0,
                    ang,
                );
            // Keep timing from standard infantry residual.
            fling_sd.sink_at_frame = sd.sink_at_frame;
            fling_sd.destroy_at_frame = sd.destroy_at_frame;
            fling_sd.phase = sd.phase;
            sd = fling_sd;
        }
        if let Some((fx, fy, fz)) = sd.take_fling_impulse() {
            self.movement.velocity.x += fx;
            self.movement.velocity.y += fy;
            self.movement.velocity.z += fz;
        }
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
        if !sd.tick(current_frame) {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        true
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
        if let Some(msec) =
            crate::game_logic::host_lifetime_update::lifetime_msec_for_template(&self.template_name)
        {
            self.lifetime_update = Some(
                crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_msec(
                    current_frame,
                    msec,
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
                // previous health residual ≈ current before death; use max-current.
                let max_h = self.health.maximum.max(self.max_health).max(1.0);
                let prev = self.health.current.max(0.0);
                self.create_object_die_transfer_damage = (max_h - prev).max(0.0);
            }
        }
    }

    pub fn take_pending_create_object_die_spawns(&mut self) -> (Vec<String>, f32, bool) {
        let spawns = std::mem::take(&mut self.pending_create_object_die_spawns);
        let dmg = self.create_object_die_transfer_damage;
        self.create_object_die_transfer_damage = 0.0;
        let transfer = self
            .create_object_die
            .as_ref()
            .map(|c| c.transfer_previous_health)
            .unwrap_or(false);
        (spawns, dmg, transfer)
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

    /// C++ FXListDie::onDie residual.
    pub fn fire_fx_list_die(&mut self) {
        self.ensure_fx_list_die();
        let Some(fx) = self.fx_list_die.as_mut() else {
            return;
        };
        if let Some((f, a)) = fx.on_die() {
            if self.pending_death_fx.is_none() {
                self.pending_death_fx = f;
            }
            if self.pending_death_audio.is_none() {
                self.pending_death_audio = a;
            }
        }
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
        let Some(st) = self.structure_topple_data.as_mut() else {
            return false;
        };
        if !st.tick(current_frame) {
            return false;
        }
        // doToppleDoneStuff residual: finalize death.
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
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
            self.topple_data = Some(crate::game_logic::host_topple::HostToppleData::default());
        }
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
        let kill_now = {
            let Some(td) = self.topple_data.as_mut() else {
                return false;
            };
            if !td.is_able_to_be_toppled() {
                return false;
            }
            td.apply_toppling_force(dir_x, dir_y, topple_speed, options)
        };
        if kill_now {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        }
        kill_now
    }

    /// C++ ToppleUpdate::update residual. Returns true if death-by-topple this frame.
    pub fn tick_topple(&mut self) -> bool {
        let Some(td) = self.topple_data.as_mut() else {
            return false;
        };
        if !td.tick() {
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
