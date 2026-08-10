use super::*;

impl Object {
    /// C++ JetSlowDeathBehavior residual begin.
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
        let hat = self.get_position().y; // residual vs terrain 0
        let mut j = crate::game_logic::host_jet_slow_death::HostJetSlowDeathData::default();
        j.begin(hat.max(0.0));
        self.jet_slow_death = Some(j);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    /// Tick jet crash. `terrain_height` world Y of ground.
    pub fn tick_jet_slow_death(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        let pos = self.get_position();
        let hat = (pos.y - terrain_height).max(0.0);
        let ori = self.get_orientation();
        let Some(j) = self.jet_slow_death.as_mut() else {
            return false;
        };
        if !j.is_active() {
            return false;
        }
        let (dy, d_roll, done) = j.tick(current_frame, hat);
        let mut np = pos;
        np.y = (np.y + dy).max(terrain_height);
        self.set_position(np);
        // Use orientation as roll residual peel (presentation).
        self.set_orientation(ori + d_roll);
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
}
