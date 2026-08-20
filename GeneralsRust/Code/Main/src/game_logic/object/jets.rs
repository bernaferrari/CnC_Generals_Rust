use super::*;

impl Object {
    /// C++ JetSlowDeathBehavior residual begin.
    /// Returns true when air crash should defer destroy.
    pub fn begin_jet_slow_death(&mut self) -> bool {
        if !crate::game_logic::host_jet_slow_death::is_jet_slow_death_template(&self.template_name)
        {
            return false;
        }
        if self
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active() || j.done)
            .unwrap_or(false)
        {
            return self
                .jet_slow_death
                .as_ref()
                .map(|j| j.is_active())
                .unwrap_or(false);
        }
        let hat = self.get_position().y.max(0.0);
        let deck = self.status.deck_height_offset;
        let mut j = crate::game_logic::host_jet_slow_death::HostJetSlowDeathData::with_fx(
            crate::game_logic::host_jet_slow_death::jet_slow_death_fx_for_template(
                &self.template_name,
            ),
        );
        j.begin(hat, deck);
        self.apply_jet_death_phase(j.take_pending_effects());
        if j.started_on_ground {
            self.status.deck_height_offset = false;
            self.jet_slow_death = Some(j);
            return false;
        }
        self.jet_slow_death = Some(j);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.status.deck_height_offset = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    fn apply_jet_death_phase(
        &mut self,
        ev: crate::game_logic::host_jet_slow_death::HostJetDeathPhaseEvent,
    ) {
        if let Some(fx) = ev.fx {
            if self.pending_death_fx.is_none() {
                self.pending_death_fx = Some(fx);
            }
        }
        if let Some(audio) = ev.audio {
            if self.pending_death_audio.is_none() {
                self.pending_death_audio = Some(audio);
            }
        }
        if let Some(ocl) = ev.ocl {
            let templates =
                crate::game_logic::host_create_object_die::peel_ocl_spawn_templates(&ocl);
            if !templates.is_empty() {
                self.pending_create_object_die_spawns.extend(templates);
            }
        }
        let _ = ev.stop_loop;
    }

    /// Tick jet crash. `terrain_height` world Y of ground.
    pub fn tick_jet_slow_death(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        let pos = self.get_position();
        let hat = (pos.y - terrain_height).max(0.0);
        let ori = self.get_orientation();
        let Some(j) = self.jet_slow_death.as_mut() else {
            return false;
        };
        if !j.is_active() && !j.started_on_ground {
            return false;
        }
        let (dy, d_roll, done) = j.tick(current_frame, hat);
        let ev = j.take_pending_effects();
        let mut np = pos;
        np.y = (np.y + dy).max(terrain_height);
        self.set_position(np);
        self.set_orientation(ori + d_roll);
        self.apply_jet_death_phase(ev);
        if done {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ HelicopterSlowDeathBehavior residual begin.
    pub fn begin_helicopter_slow_death(&mut self) -> bool {
        if !crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &self.template_name,
        ) {
            return false;
        }
        if self
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active() || h.done)
            .unwrap_or(false)
        {
            return self
                .helicopter_slow_death
                .as_ref()
                .map(|h| h.is_active())
                .unwrap_or(false);
        }
        let mut h =
            crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData::default();
        h.begin();
        self.helicopter_slow_death = Some(h);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    /// Returns true when heli crash finished and should destroy.
    pub fn tick_helicopter_slow_death(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        let pos = self.get_position();
        let hat = (pos.y - terrain_height).max(0.0);
        let ori = self.get_orientation();
        let Some(h) = self.helicopter_slow_death.as_mut() else {
            return false;
        };
        if !h.is_active() {
            return false;
        }
        let (dx, dy, dz, dori, done, _blade) = h.tick(current_frame, hat);
        let mut np = pos;
        np.x += dx;
        np.y = (np.y + dy).max(terrain_height);
        np.z += dz;
        self.set_position(np);
        self.set_orientation(ori + dori);
        if done {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ `JetAIUpdate::getProducerLocation` — capture once.
    pub fn capture_jet_producer_location(&mut self, airfield_pos: Option<Vec3>) {
        if self.jet_producer_location.is_some() {
            return;
        }
        let p = airfield_pos.unwrap_or_else(|| self.get_position());
        self.jet_producer_location = Some([p.x, p.y, p.z]);
    }

    pub fn jet_producer_location_vec(&self) -> Option<Vec3> {
        self.jet_producer_location
            .map(|p| Vec3::new(p[0], p[1], p[2]))
    }

    /// 2D arrival at remembered producer / dead-airfield location.
    pub fn is_at_jet_producer_location(&self, arrive_radius: f32) -> bool {
        let Some(goal) = self.jet_producer_location_vec() else {
            return true;
        };
        let p = self.get_position();
        let dx = p.x - goal.x;
        let dz = p.z - goal.z;
        dx * dx + dz * dz <= arrive_radius * arrive_radius
    }

    /// C++ `JetOrHeliCirclingDeadAirfieldState::onEnter`.
    /// Returns true when this call entered the state (play VoiceLowFuel).
    pub fn enter_circling_dead_airfield(&mut self, now: u32) -> bool {
        if self.jet_circling_dead_airfield {
            return false;
        }
        // Map-placed idle jet with no producer and a full clip is not dying.
        if !self.needs_return_to_base_rearm() && self.producer_id.is_none() {
            return false;
        }
        self.jet_circling_dead_airfield = true;
        self.jet_circling_airfield_check_frame = now.saturating_add(30);
        self.target = None;
        self.set_status_attacking(false);
        self.set_status_moving(false);
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.movement.target_position = None;
        self.set_ai_state(AIState::Idle);
        true
    }

    pub fn leave_circling_dead_airfield(&mut self) {
        self.jet_circling_dead_airfield = false;
        self.jet_circling_airfield_check_frame = 0;
    }
}
