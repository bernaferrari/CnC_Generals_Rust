// Team identity, control, recruitable, script hooks
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

impl Team {
    /// Create a new team with the given ID and name
    pub fn new(name: AsciiString, id: TeamID) -> Self {
        Self {
            id,
            name,
            members: Vec::new(),
            controlling_player_id: None,
            state: String::new().into(),
            entered_or_exited: false,
            active: false,
            created: false,
            recruitable: false,
            recruitability_set: false,
            check_enemy_sighted: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            destroyed_threshold_ratio: 0.0,
            script_on_create: String::new().into(),
            script_on_idle: String::new().into(),
            script_on_enemy_sighted: String::new().into(),
            script_on_all_clear: String::new().into(),
            script_on_destroyed: String::new().into(),
            script_on_unit_destroyed: String::new().into(),
            common_attack_target: AtomicU32::new(INVALID_ID),
            current_waypoint_id: None,
            should_attempt_generic_script: [true; MAX_GENERIC_SCRIPTS],
            team_relations: None,
            player_relations: None,
            is_singleton: false,
        }
    }

    /// Get team ID
    pub fn get_id(&self) -> TeamID {
        self.id
    }

    /// Set team ID
    pub fn set_id(&mut self, id: TeamID) {
        self.id = id;
    }

    /// Get team name
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Set team name  
    pub fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// Get controlling player ID
    pub fn get_controlling_player_id(&self) -> Option<UnsignedInt> {
        self.controlling_player_id
    }

    /// Set controlling player
    pub fn set_controlling_player_id(&mut self, player_id: Option<UnsignedInt>) {
        // Always assign the controller (C++ Team::setControllingPlayer).
        // Empty leftover registry only skips the dual-world member walk.
        let changed = self.controlling_player_id != player_id;
        self.controlling_player_id = player_id;
        if !changed || dual_world_registry_unavailable() {
            return;
        }


        // C++ parity (Team::setControllingPlayer): refresh partition/shroud state of all members
        // when team control changes.
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                object_guard.handle_partition_cell_maintenance();
            });
        }
    }

    /// Get team state
    pub fn get_state(&self) -> &AsciiString {
        &self.state
    }

    /// Set team state
    pub fn set_state(&mut self, state: AsciiString) {
        self.state = state;
    }

    /// Set current waypoint ID for this team (matches C++ Team::setCurrentWaypoint).
    pub fn set_current_waypoint_id(&mut self, waypoint_id: Option<WaypointId>) {
        self.current_waypoint_id = waypoint_id;
    }

    /// Get current waypoint ID for this team.
    pub fn get_current_waypoint_id(&self) -> Option<WaypointId> {
        self.current_waypoint_id
    }

    /// Get count of targetable (alive or building) objects
    pub fn get_targetable_count(&self) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count: Int = 0;

        // C++ parity (Team::getTargetableCount):
        // count alive members that either have AI or are structures.
        for &object_id in &self.members {
            let Some(countable) = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_effectively_dead() {
                    return false;
                }
                if object_guard.get_ai_update_interface().is_none()
                    && !object_guard.is_kind_of(KindOf::Structure)
                {
                    return false;
                }
                true
            }) else {
                continue;
            };
            if countable {
                count += 1;
            }
        }

        count
    }

    /// Set team target object
    pub fn set_team_target_object(&mut self, target: ObjectID) {
        if target == INVALID_ID {
            self.common_attack_target.store(INVALID_ID, Ordering::Relaxed);
            return;
        }

        // C++ parity: only AI teams set common attack targets, and not on easy difficulty.
        let Some(controller_id) = self.controlling_player_id else {
            return;
        };
        let Some(controller_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(controller_id as Int).cloned())
        else {
            return;
        };
        let Ok(controller) = controller_arc.read() else {
            return;
        };
        if controller.get_player_type() != crate::player::PlayerType::Computer {
            return;
        }
        if controller.get_player_difficulty() == crate::player::GameDifficulty::Easy {
            return;
        }

        self.common_attack_target.store(target, Ordering::Relaxed);
    }

    /// Get team target object
    pub fn get_team_target_object(&self) -> ObjectID {
        // Wave 256: empty dual-world → invalid target.
        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        let target_id = self.common_attack_target.load(Ordering::Relaxed);
        if target_id == INVALID_ID {
            return INVALID_ID;
        }

        let Some(valid) = OBJECT_REGISTRY.with_object(target_id, |target| {
            let target_status = target.get_status_bits();
            if target_status.contains(ObjectStatusMaskType::STEALTHED)
                && !target_status.contains(ObjectStatusMaskType::DETECTED)
                && !target_status.contains(ObjectStatusMaskType::DISGUISED)
            {
                return false;
            }

            if target.is_effectively_dead() || target.get_contained_by().is_some() {
                return false;
            }

            if target.is_kind_of(KindOf::Aircraft) {
                return false;
            }

            true
        }) else {
            self.common_attack_target.store(INVALID_ID, Ordering::Relaxed);
            return INVALID_ID;
        };
        if valid {
            target_id
        } else {
            self.common_attack_target.store(INVALID_ID, Ordering::Relaxed);
            INVALID_ID
        }
    }

    /// Whether this team should share a common attack target.
    pub fn attack_common_target(&self) -> Bool {
        let team_name = self.get_name().to_string();
        let Ok(factory) = get_team_factory().lock() else {
            return true;
        };
        factory
            .find_team_prototype(&team_name)
            .map(|prototype| prototype.attack_common_target())
            .unwrap_or(true)
    }

    /// Access team member list (read-only).
    /// Set team as active
    pub fn set_active(&mut self) {
        if !self.active {
            self.created = true;
            self.active = true;
        }
    }

    /// Check if team is active
    pub fn is_active(&self) -> Bool {
        self.active
    }

    pub fn get_see_enemy(&self) -> Bool {
        self.see_enemy
    }

    pub fn get_prev_see_enemy(&self) -> Bool {
        self.prev_see_enemy
    }

    pub fn get_was_idle(&self) -> Bool {
        self.was_idle
    }

    pub fn get_destroy_threshold(&self) -> Int {
        self.destroy_threshold
    }

    pub fn get_cur_units_count(&self) -> Int {
        self.cur_units
    }

    /// C++ `Team::xfer` script latches. Must not go through `set_active`
    /// (that forces `created=true` and re-fires OnCreate after load).
    pub fn restore_save_script_state(
        &mut self,
        created: Bool,
        active: Bool,
        see_enemy: Bool,
        prev_see_enemy: Bool,
        was_idle: Bool,
        destroy_threshold: Int,
        cur_units: Int,
        waypoint_id: Option<WaypointId>,
        generic_attempts: &[Bool],
        recruitability_set: Bool,
        recruitable: Bool,
        state: &str,
    ) {
        self.created = created;
        self.active = active;
        self.see_enemy = see_enemy;
        self.prev_see_enemy = prev_see_enemy;
        self.was_idle = was_idle;
        self.destroy_threshold = destroy_threshold;
        self.cur_units = cur_units;
        self.current_waypoint_id = waypoint_id;
        for (i, &flag) in generic_attempts.iter().enumerate().take(MAX_GENERIC_SCRIPTS) {
            self.should_attempt_generic_script[i] = flag;
        }
        self.recruitability_set = recruitability_set;
        self.recruitable = recruitable;
        self.state = state.to_string().into();
    }


    /// Check if this team can be recruited by AI/team-building logic.
    pub fn is_recruitable(&self) -> Bool {
        self.recruitable
    }

    /// Returns true if this team has an explicit runtime recruitability override.
    /// Mirrors C++ Team::m_isRecruitablitySet semantics.
    pub fn is_recruitability_set(&self) -> Bool {
        self.recruitability_set
    }

    /// Seed recruitability from prototype defaults without marking a runtime override.
    fn set_prototype_recruitable(&mut self, recruitable: Bool) {
        self.recruitable = recruitable;
        self.recruitability_set = false;
    }

    /// Set whether this team can be recruited.
    pub fn set_recruitable(&mut self, recruitable: Bool) {
        self.recruitable = recruitable;
        self.recruitability_set = true;
    }

    /// Copy script hook fields from the team template.
    pub fn apply_template_script_hooks(&mut self, prototype: &TeamPrototype) {
        self.script_on_create = prototype.get_script_on_create().clone();
        self.script_on_idle = prototype.get_script_on_idle().clone();
        self.script_on_enemy_sighted = prototype.get_script_on_enemy_sighted().clone();
        self.script_on_all_clear = prototype.get_script_on_all_clear().clone();
        self.script_on_destroyed = prototype.get_script_on_destroyed().clone();
        self.script_on_unit_destroyed = prototype.get_script_on_unit_destroyed().clone();
        self.destroyed_threshold_ratio = prototype.get_destroyed_threshold();
        self.check_enemy_sighted =
            !self.script_on_enemy_sighted.is_empty() || !self.script_on_all_clear.is_empty();
    }

    pub fn should_attempt_generic_script(&self, index: usize) -> Bool {
        self.should_attempt_generic_script
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    pub fn disable_generic_script_attempt(&mut self, index: usize) {
        if let Some(should_attempt) = self.should_attempt_generic_script.get_mut(index) {
            *should_attempt = false;
        }
    }
}
