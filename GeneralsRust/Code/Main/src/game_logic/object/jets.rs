use super::*;

/// C++ `JetAIUpdate` live-host residual (Wave 12).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostJetAi {
    /// C++ `m_returnToBaseFrame` (0 = unset).
    pub return_to_base_frame: u32,
    /// C++ `m_attackersMissExpireFrame`.
    pub attackers_miss_expire_frame: u32,
    /// C++ `m_attackLocoExpireFrame`.
    #[serde(default)]
    pub attack_loco_expire_frame: u32,
    /// C++ `AIUpdateInterface::getCurLocomotorSetType` residual for jets.
    #[serde(default)]
    pub cur_locomotor_set: Option<String>,
    /// C++ `ALLOW_AIR_LOCO`. Ground taxi / hangar creation keeps this false.
    #[serde(default)]
    pub allow_air_loco: bool,

    /// C++ `m_untargetableExpireFrame`.
    pub untargetable_expire_frame: u32,
    /// C++ `m_targetedBy`.
    pub targeted_by: Vec<ObjectId>,
    /// C++ `ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD`.
    pub allow_interrupt_for_reload: bool,
    /// C++ `HAS_PENDING_COMMAND`.
    pub has_pending_command: bool,
    /// Guard/hunt reconstitution after `RELOAD_AMMO`.
    pub pending_resume: HostJetPendingResume,
    /// C++ `TAKEOFF_IN_PROGRESS`.
    pub takeoff_in_progress: bool,
    /// C++ `LANDING_IN_PROGRESS`.
    pub landing_in_progress: bool,
    /// C++ afterburner model/sound latch.
    pub afterburners_on: bool,
    /// C++ `JetPauseBeforeTakeoffState::m_when`.
    pub takeoff_pause_until: u32,
    /// C++ `m_whenTransfer`.
    pub takeoff_pause_transfer: u32,
    pub takeoff_pause_armed: bool,
    /// Locomotor max lift captured on takeoff enter.
    pub takeoff_max_lift: f32,
    pub takeoff_runway_end: Option<[f32; 3]>,
    pub takeoff_runway_dist: f32,
    /// C++ `JetOrHeliTaxiState` TAXI_FROM_HANGAR / TAXI_TO_TAKEOFF.
    #[serde(default)]
    pub taxi_to_takeoff: bool,
    /// Runway-head waypoint used to start PauseBeforeTakeoff.
    #[serde(default)]
    pub takeoff_runway_start: Option<[f32; 3]>,
    /// C++ `m_waitedForTaxiID` was-in-line latch captured at taxi start.
    #[serde(default)]
    pub takeoff_waited_for_taxi: bool,
    /// C++ RETURNING_FOR_LANDING / LANDING / TAXI_FROM_LANDING.
    /// 0 none, 1 fly to approach, 2 land runway, 3 taxi to parking.
    #[serde(default)]
    pub rtb_landing_phase: u8,
    /// C++ `m_afterburnerSound.isCurrentlyPlaying`.
    #[serde(default)]
    pub afterburner_sound_playing: bool,
    /// C++ lock-on drawable world position.
    pub lockon_pos: Option<[f32; 3]>,
    pub lockon_hidden: bool,
    pub lockon_tick_pending: bool,
    /// C++ `JetAIUpdate::m_lockonDrawable` objectless DrawableID.
    #[serde(default)]
    pub lockon_drawable_id: Option<u32>,
    /// C++ `JetTakeoffOrLandingState::m_landingSoundPlayed`.
    #[serde(default)]
    pub landing_sound_played: bool,
}

/// Pending command reconstituted after airfield reload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostJetPendingResume {
    #[default]
    None,
    GuardArea,
    GuardObject,
    Hunt,
    GuardRetaliate,
}

/// Retail AmericaJetStealthFighter `LockonTime` 1500ms → 45 frames.
pub const STEALTH_FIGHTER_LOCKON_TIME_MS: u32 = 1500;
pub const STEALTH_FIGHTER_LOCKON_TIME_FRAMES: u32 = 45;
pub const STEALTH_FIGHTER_LOCKON_CURSOR: &str = "GenericLockon";
pub const STEALTH_FIGHTER_LOCKON_INITIAL_DIST: f32 = 300.0;
pub const STEALTH_FIGHTER_LOCKON_FREQ: f32 = 0.5;
pub const STEALTH_FIGHTER_LOCKON_ANGLE_SPIN: f32 = 1440.0_f32.to_radians();
pub const STEALTH_FIGHTER_LOCKON_BLINKY: bool = true;
/// Retail `TakeoffPause` 500ms → 15 frames for runway jets.
pub const JET_TAKEOFF_PAUSE_MS: u32 = 500;
pub const JET_TAKEOFF_PAUSE_FRAMES: u32 = 15;
/// MiscAudio.ini slot; queue `resolve_misc_audio_event` playable name, not this token.
pub const JET_LOCKON_TICK_SOUND: &str = "LockonTickSound";
/// UnitSpecificSounds slot; queue `resolve_per_unit_sound` playable name, not this token.
pub const JET_AFTERBURNER_SOUND: &str = "Afterburner";
pub const JET_RTB_PHASE_APPROACH: u8 = 1;
pub const JET_RTB_PHASE_LANDING: u8 = 2;
pub const JET_RTB_PHASE_TAXI: u8 = 3;
/// C++ `TheAudio->removeAudioEvent` counterpart for Afterburner.
pub const JET_AFTERBURNER_SOUND_STOP: &str = "AfterburnerStop";
/// MiscAudio.ini slot; queue `resolve_misc_audio_event` playable name, not this token.
pub const JET_WHEEL_SCREECH_SOUND: &str = "AircraftWheelScreech";
/// C++ `zSlop = 0.25f` first-contact slop (JetAIUpdate.cpp:822).
pub const JET_WHEEL_SCREECH_Z_SLOP: f32 = 0.25;

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
            crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData::with_fx(
                crate::game_logic::host_helicopter_slow_death::heli_slow_death_fx_for_template(
                    &self.template_name,
                ),
            );
        h.begin();
        self.apply_heli_death_phase(h.take_pending_effects());
        self.helicopter_slow_death = Some(h);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    pub(crate) fn apply_heli_death_phase(
        &mut self,
        ev: crate::game_logic::host_helicopter_slow_death::HostHeliDeathPhaseEvent,
    ) {
        if let Some(fx) = ev.fx {
            self.pending_death_fx = Some(fx);
        }
        if ev.stop_loop {
            self.pending_death_audio_stop = true;
            if self.pending_death_audio.is_none() {
                if let Some(loop_name) = self
                    .helicopter_slow_death
                    .as_ref()
                    .and_then(|h| h.fx.death_loop_sound.clone())
                {
                    self.pending_death_audio = Some(loop_name);
                }
            }
        } else if let Some(audio) = ev.audio {
            if self.pending_death_audio.is_none() {
                self.pending_death_audio = Some(audio);
            }
        }
        if let Some(ocl) = ev.ocl {
            let templates =
                crate::game_logic::host_create_object_die::peel_ocl_spawn_templates(&ocl);
            if !templates.is_empty() {
                self.pending_create_object_die_spawns.extend(templates);
            } else {
                self.pending_create_object_die_spawns.push(ocl);
            }
        }
        if let Some(rubble) = ev.rubble {
            if !rubble.is_empty() && !rubble.eq_ignore_ascii_case("none") {
                self.pending_create_object_die_spawns.push(rubble);
            }
        }
        if ev.held {
            self.set_status_disabled_held(true);
        }
        if ev.special_damaged {
            let bit = crate::game_logic::host_enum_table_residual::special_damaged_model_bit();
            self.model_condition_bits |= 1u128 << bit;
        }
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
        let ev = h.take_pending_effects();
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
            self.apply_heli_death_phase(ev);
            return true;
        }
        self.apply_heli_death_phase(ev);
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

    pub fn is_runway_jet(&self) -> bool {
        (self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft)
            && !crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
                &self.template_name,
            )
    }

    pub fn jet_return_to_base_idle_frames(&self) -> u32 {
        if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(&self.template_name) {
            crate::game_logic::host_aurora_bomb::AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES
        } else if self.is_runway_jet() {
            // Retail Raptor / MIG / Stealth share Aurora's 10000ms idle RTB.
            crate::game_logic::host_aurora_bomb::AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES
        } else {
            0
        }
    }

    pub fn jet_sneaky_offset_when_attacking(&self) -> f32 {
        if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(&self.template_name) {
            crate::game_logic::host_aurora_bomb::AURORA_JET_SNEAKY_OFFSET
        } else {
            0.0
        }
    }

    pub fn jet_attackers_miss_persist_frames(&self) -> u32 {
        if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(&self.template_name) {
            // 2000ms → 60 frames @ 30 FPS.
            60
        } else {
            0
        }
    }

    /// Retail `AttackLocomotorType = SET_SUPERSONIC` jets (runway attack craft).
    pub fn jet_uses_attack_locomotor(&self) -> bool {
        self.is_runway_jet()
    }

    /// C++ `m_attackLocoPersistTime` in logic frames. Retail 100ms on Aurora /
    /// Raptor / MIG / Stealth (`AttackLocomotorPersistTime`).
    pub fn jet_attack_loco_persist_frames(&self) -> u32 {
        if !self.jet_uses_attack_locomotor() {
            return 0;
        }
        crate::game_logic::host_aurora_bomb::aurora_ms_to_frames(
            crate::game_logic::host_aurora_bomb::AURORA_JET_ATTACK_LOCO_PERSIST_MS,
        )
    }

    /// C++ `JetAIUpdate` `ALLOW_AIR_LOCO` residual.
    pub(crate) fn jet_allows_air_loco(&self) -> bool {
        if self.contained_by.is_some()
            || matches!(self.ai_state, AIState::Docked | AIState::Docking)
        {
            return false;
        }
        if self.jet_ai.allow_air_loco {
            return true;
        }
        // Map-placed airborne jets never taxied; they still use air loco.
        self.status.airborne_target && !self.jet_ai.landing_in_progress
    }

    /// C++ `getCurLocomotorSetType` token (jets + RiderChange).
    pub fn get_cur_locomotor_set_token(&self) -> Option<&str> {
        if let Some(set) = self.jet_ai.cur_locomotor_set.as_deref() {
            if !set.is_empty() {
                return Some(set);
            }
        }
        self.rider_change_locomotor_set.as_deref()
    }

    /// C++ `friend_setAllowAirLoco(false)` + `chooseLocomotorSet(TAXIING)`.
    pub fn apply_taxiing_locomotor_set(&mut self) {
        self.jet_ai.allow_air_loco = false;
        self.choose_jet_locomotor_set("SET_TAXIING");
        // C++ JetOrHeliTaxiState::onEnter setAllowInvalidPosition
        // (JetAIUpdate.cpp:617) so stall/runway cells skip 3x3 shove.
        self.set_allow_invalid_position(true);
    }

    /// C++ takeoff/airborne `friend_setAllowAirLoco(true)` + NORMAL/SUPERSONIC.
    pub fn apply_airborne_locomotor_set(&mut self) {
        self.jet_ai.allow_air_loco = true;
        if self.jet_ai.attack_loco_expire_frame != 0 {
            self.choose_jet_locomotor_set("SET_SUPERSONIC");
        } else {
            self.choose_jet_locomotor_set("SET_NORMAL");
        }
    }

    /// C++ `JetAIUpdate::chooseLocomotorSet`.
    fn choose_jet_locomotor_set(&mut self, set: &str) {
        use crate::game_logic::host_upgrade_module_residuals::{
            HostLocomotorSetKind, apply_locomotor_set_kind,
        };
        let kind = if set.eq_ignore_ascii_case("SET_TAXIING") {
            HostLocomotorSetKind::Taxiing
        } else if set.eq_ignore_ascii_case("SET_SUPERSONIC") {
            HostLocomotorSetKind::Supersonic
        } else {
            HostLocomotorSetKind::Normal
        };
        let already = self
            .jet_ai
            .cur_locomotor_set
            .as_deref()
            .is_some_and(|cur| cur.eq_ignore_ascii_case(set));
        if already {
            return;
        }
        let _ = apply_locomotor_set_kind(self, kind);
    }

    /// C++ `JetAIUpdate.cpp:1950-1981` persist + `chooseLocomotorSet(attackingLoco)`.
    fn tick_jet_attack_locomotor(&mut self, now: u32) {
        if !self.jet_uses_attack_locomotor() {
            return;
        }
        if self.status.attacking {
            self.jet_ai.attack_loco_expire_frame =
                now.saturating_add(self.jet_attack_loco_persist_frames());
        } else if self.jet_ai.attack_loco_expire_frame != 0
            && now >= self.jet_ai.attack_loco_expire_frame
        {
            self.jet_ai.attack_loco_expire_frame = 0;
        }
        // `chooseLocomotorSet` remaps to TAXIING when `!ALLOW_AIR_LOCO`.
        if !self.jet_allows_air_loco() {
            self.choose_jet_locomotor_set("SET_TAXIING");
            return;
        }
        if self.jet_ai.attack_loco_expire_frame != 0 {
            self.choose_jet_locomotor_set("SET_SUPERSONIC");
        } else if self.jet_ai.cur_locomotor_set.as_deref().is_some_and(|s| {
            s.to_ascii_uppercase().contains("SUPERSONIC") || s.eq_ignore_ascii_case("SET_TAXIING")
        }) {
            self.choose_jet_locomotor_set("SET_NORMAL");
        }
    }

    pub fn jet_lockon_time_frames(&self) -> u32 {
        if crate::game_logic::host_stealth_fighter::is_stealth_fighter_template(&self.template_name)
        {
            STEALTH_FIGHTER_LOCKON_TIME_FRAMES
        } else {
            0
        }
    }

    pub fn jet_takeoff_pause_frames(&self) -> u32 {
        if self.is_runway_jet() {
            JET_TAKEOFF_PAUSE_FRAMES.max(1)
        } else {
            1
        }
    }

    pub fn is_jet_guard_or_hunt(&self) -> bool {
        self.hunting
            || matches!(
                self.ai_state,
                AIState::GuardingArea
                    | AIState::GuardingObject
                    | AIState::GuardRetaliating
                    | AIState::Patrolling
            )
    }

    pub fn mark_jet_command_for_reload_interrupt(&mut self, allow: bool) {
        if !self.is_runway_jet() {
            return;
        }
        self.jet_ai.allow_interrupt_for_reload = allow;
        if allow {
            self.jet_ai.pending_resume =
                if self.hunting || matches!(self.ai_state, AIState::Patrolling) {
                    HostJetPendingResume::Hunt
                } else if matches!(self.ai_state, AIState::GuardingObject) {
                    HostJetPendingResume::GuardObject
                } else if matches!(self.ai_state, AIState::GuardRetaliating) {
                    HostJetPendingResume::GuardRetaliate
                } else {
                    HostJetPendingResume::GuardArea
                };
        } else if !self.jet_ai.has_pending_command {
            self.jet_ai.pending_resume = HostJetPendingResume::None;
        }
    }

    /// C++ `JetAIUpdate::getSneakyTargetingOffset`.
    pub fn get_sneaky_targeting_offset(&self, now: u32) -> Option<Vec3> {
        let expire = self.jet_ai.attackers_miss_expire_frame;
        if expire == 0 || now >= expire {
            return None;
        }
        let offset = self.jet_sneaky_offset_when_attacking();
        if offset.abs() < f32::EPSILON {
            return None;
        }
        let dir = self.unit_direction_vector_2d();
        Some(Vec3::new(dir.x * offset, 0.0, dir.y * offset))
    }

    pub fn apply_sneaky_targeting_offset(&self, pos: Vec3, now: u32) -> Vec3 {
        match self.get_sneaky_targeting_offset(now) {
            Some(off) => pos + off,
            None => pos,
        }
    }

    /// C++ `JetAIUpdate::addTargeter`.
    pub fn add_jet_targeter(&mut self, id: ObjectId, add: bool, now: u32) {
        let lockon_time = self.jet_lockon_time_frames();
        if lockon_time == 0 {
            return;
        }
        if add {
            if !self.jet_ai.targeted_by.contains(&id) {
                self.jet_ai.targeted_by.push(id);
                if self.jet_ai.untargetable_expire_frame == 0 && self.jet_ai.targeted_by.len() == 1
                {
                    self.jet_ai.untargetable_expire_frame = now.saturating_add(lockon_time);
                }
            }
        } else if let Some(pos) = self
            .jet_ai
            .targeted_by
            .iter()
            .position(|entry| *entry == id)
        {
            self.jet_ai.targeted_by.remove(pos);
            if self.jet_ai.targeted_by.is_empty() {
                self.jet_ai.untargetable_expire_frame = 0;
                self.jet_ai.lockon_pos = None;
            }
        }
    }

    /// C++ `JetAIUpdate::isTemporarilyPreventingAimSuccess`.
    pub fn is_temporarily_preventing_aim_success(&self, now: u32) -> bool {
        self.jet_ai.untargetable_expire_frame != 0 && now < self.jet_ai.untargetable_expire_frame
    }

    /// C++ `JetAIUpdate::notifyVictimIsDead`.
    pub fn notify_jet_victim_is_dead(&mut self, now: u32) {
        if self.is_runway_jet() {
            self.jet_ai.return_to_base_frame = now;
        }
    }

    pub fn enable_jet_afterburners(&mut self, enable: bool) -> bool {
        let changed = self.jet_ai.afterburners_on != enable;
        self.jet_ai.afterburners_on = enable;
        let bit = crate::game_logic::host_enum_table_residual::jetafterburner_model_bit();
        if bit < 128 {
            if enable {
                self.model_condition_bits |= 1u128 << bit;
            } else {
                self.model_condition_bits &= !(1u128 << bit);
            }
        }
        changed && enable
    }

    /// C++ `JetAIUpdate::update` JETEXHAUST: velocity > 0 AND ALLOW_AIR_LOCO.
    fn stamp_jet_exhaust(&mut self) {
        let bit = crate::game_logic::host_enum_table_residual::jetexhaust_model_bit();
        if bit >= 128 {
            return;
        }
        let speed = self.movement.velocity.length();
        if speed > 0.0 && self.jet_allows_air_loco() {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
    }

    /// C++ TAXI_FROM_HANGAR / TAXI_TO_TAKEOFF: afterburners stay off.
    pub fn arm_jet_taxi_to_takeoff(
        &mut self,
        runway_start: Vec3,
        runway_end: Vec3,
        runway_dist: f32,
        waited_for_taxi: bool,
    ) {
        self.jet_ai.taxi_to_takeoff = true;
        self.jet_ai.takeoff_in_progress = false;
        self.jet_ai.takeoff_pause_armed = false;
        self.jet_ai.takeoff_runway_start = Some([runway_start.x, runway_start.y, runway_start.z]);
        self.jet_ai.takeoff_runway_end = Some([runway_end.x, runway_end.y, runway_end.z]);
        self.jet_ai.takeoff_runway_dist = runway_dist.max(1.0);
        self.jet_ai.takeoff_waited_for_taxi = waited_for_taxi;
        self.apply_taxiing_locomotor_set();
        // C++ JetOrHeliTaxiState / JetAwaitingRunwayState (JetAIUpdate.cpp:259-260, :615-616).
        self.set_precise_z_and_ultra_accurate(true);
        let _ = self.enable_jet_afterburners(false);
    }

    pub fn jet_reached_runway_head(&self) -> bool {
        if !self.jet_ai.taxi_to_takeoff {
            return false;
        }
        let Some(start) = self.jet_ai.takeoff_runway_start else {
            return self.movement.path.is_empty()
                || self.movement.current_path_index >= self.movement.path.len();
        };
        let start = Vec3::new(start[0], start[1], start[2]);
        let pos = self.get_position();
        let dx = pos.x - start.x;
        let dz = pos.z - start.z;
        dx * dx + dz * dz <= 12.0 * 12.0
    }

    pub fn begin_jet_runway_takeoff(
        &mut self,
        now: u32,
        runway_end: Vec3,
        runway_dist: f32,
        waited_for_taxi: bool,
    ) {
        self.jet_ai.takeoff_in_progress = true;
        self.jet_ai.takeoff_pause_armed = true;
        self.jet_ai.takeoff_pause_until = now.saturating_add(self.jet_takeoff_pause_frames());
        self.jet_ai.takeoff_pause_transfer =
            now.saturating_add(if waited_for_taxi { 2 } else { 1 });
        if self.jet_ai.takeoff_max_lift <= 0.0 {
            self.jet_ai.takeoff_max_lift = self.max_lift.max(self.get_max_lift());
        }
        self.jet_ai.takeoff_runway_end = Some([runway_end.x, runway_end.y, runway_end.z]);
        self.jet_ai.takeoff_runway_dist = runway_dist.max(1.0);
        self.max_lift = 0.0;
        self.apply_taxiing_locomotor_set();
        // C++ JetTakeoffOrLandingState::onEnter (JetAIUpdate.cpp:725-726).
        self.set_precise_z_and_ultra_accurate(true);
        let _ = self.enable_jet_afterburners(true);
    }

    /// C++ `JetTakeoffOrLandingState::update` takeoff lift ramp.
    pub fn tick_jet_takeoff_lift(&mut self, now: u32) -> bool {
        if !self.jet_ai.takeoff_in_progress {
            return false;
        }
        if self.jet_ai.takeoff_pause_armed && now < self.jet_ai.takeoff_pause_until {
            self.max_lift = 0.0;
            return false;
        }
        if !self.jet_ai.allow_air_loco {
            self.apply_airborne_locomotor_set();
            // C++ chooseLocomotorSet then setUsePreciseZPos (JetAIUpdate.cpp:709,725-726).
            self.set_precise_z_and_ultra_accurate(true);
        }
        let Some(end) = self.jet_ai.takeoff_runway_end else {
            return true;
        };
        let end = Vec3::new(end[0], end[1], end[2]);
        let pos = self.get_position();

        let dist = (end - pos).length();
        let mut ratio = 1.0 - (dist / self.jet_ai.takeoff_runway_dist.max(1.0));
        ratio *= ratio;
        ratio = ratio.clamp(0.0, 1.0);
        let base = self.jet_ai.takeoff_max_lift.max(0.0);
        self.max_lift = base * ratio;
        if dist <= 8.0 || ratio >= 0.98 {
            self.finish_jet_takeoff();
            return true;
        }
        false
    }

    pub fn finish_jet_takeoff(&mut self) {
        self.jet_ai.takeoff_in_progress = false;
        self.jet_ai.takeoff_pause_armed = false;
        self.jet_ai.takeoff_pause_until = 0;
        self.jet_ai.takeoff_pause_transfer = 0;
        self.jet_ai.takeoff_runway_end = None;
        if self.jet_ai.takeoff_max_lift > 0.0 {
            self.max_lift = self.jet_ai.takeoff_max_lift;
        }
        self.apply_airborne_locomotor_set();
        // C++ JetTakeoffOrLandingState::onExit (JetAIUpdate.cpp:886-887, :662-663).
        self.set_precise_z_and_ultra_accurate(false);
        self.set_allow_invalid_position(false);
        let _ = self.enable_jet_afterburners(false);
        self.jet_ai.taxi_to_takeoff = false;
        self.jet_ai.takeoff_runway_start = None;
        self.jet_ai.takeoff_waited_for_taxi = false;
    }

    pub fn jet_should_transfer_runway(&self, now: u32) -> bool {
        if !self.jet_ai.takeoff_in_progress {
            return false;
        }
        if self.jet_ai.takeoff_pause_armed {
            return now >= self.jet_ai.takeoff_pause_transfer;
        }
        true
    }

    /// C++ idle / reload empty-clip RTB + idle timer + sneaky persist + lockon.
    pub fn tick_jet_ai_update(&mut self, now: u32) -> JetAiTickAction {
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return JetAiTickAction::None;
        }
        if !self.jet_allows_air_loco() {
            self.apply_taxiing_locomotor_set();
        }
        if self.jet_ai.afterburners_on {
            let _ = self.enable_jet_afterburners(true);
        }
        let _ = self.tick_jet_takeoff_lift(now);
        self.stamp_jet_exhaust();
        self.position_jet_lockon(now);

        if self.status.attacking {
            let persist = self.jet_attackers_miss_persist_frames();
            if persist > 0 {
                self.jet_ai.attackers_miss_expire_frame = now.saturating_add(persist);
            }
        } else if self.jet_ai.attackers_miss_expire_frame != 0
            && now >= self.jet_ai.attackers_miss_expire_frame
        {
            self.jet_ai.attackers_miss_expire_frame = 0;
        }
        self.tick_jet_attack_locomotor(now);

        let airborne = self.status.airborne_target
            && self.contained_by.is_none()
            && !matches!(
                self.ai_state,
                AIState::Docked | AIState::Docking | AIState::Entering
            );
        if !airborne {
            self.jet_ai.return_to_base_frame = 0;
            if self.contained_by.is_some() {
                self.jet_ai.landing_in_progress = false;
                if self.jet_ai.has_pending_command
                    && self.jet_ai.pending_resume != HostJetPendingResume::None
                    && !self.needs_return_to_base_rearm()
                    && self.airfield_rearm_ready_frame.is_none()
                {
                    return JetAiTickAction::ResumePending;
                }
            }
            return JetAiTickAction::None;
        }

        // C++ arms the idle RTB timer only under AIUpdateInterface::isIdle()
        // (JetAIUpdate.cpp:1831, 1888): an attacking jet is not idle, so its
        // timer is cleared by the non-idle branch (JetAIUpdate.cpp:1899) and
        // never armed mid-attack.
        let idle = matches!(self.ai_state, AIState::Idle)
            && !self.status.attacking
            && self.target.is_none()
            && !self.hunting
            && self.guard_position.is_none()
            && self.guard_target.is_none();

        if idle {
            if self.needs_return_to_base_rearm() {
                self.jet_ai.return_to_base_frame = 0;
                return JetAiTickAction::ReturnToBase;
            }
            if self.jet_ai.has_pending_command
                && self.jet_ai.pending_resume != HostJetPendingResume::None
                && !self.needs_return_to_base_rearm()
            {
                return JetAiTickAction::ResumePending;
            }
            if self.jet_ai.return_to_base_frame != 0 && now >= self.jet_ai.return_to_base_frame {
                self.jet_ai.return_to_base_frame = 0;
                return JetAiTickAction::ReturnToBase;
            }
            if self.jet_ai.return_to_base_frame == 0 {
                let idle_time = self.jet_return_to_base_idle_frames();
                if idle_time > 0 {
                    self.jet_ai.return_to_base_frame = now.saturating_add(idle_time);
                }
            }
            return JetAiTickAction::None;
        }

        self.jet_ai.return_to_base_frame = 0;
        if self.jet_ai.allow_interrupt_for_reload && self.needs_return_to_base_rearm() {
            self.jet_ai.has_pending_command = true;
            if self.jet_ai.pending_resume == HostJetPendingResume::None {
                self.jet_ai.pending_resume =
                    if self.hunting || matches!(self.ai_state, AIState::Patrolling) {
                        HostJetPendingResume::Hunt
                    } else if matches!(self.ai_state, AIState::GuardingObject) {
                        HostJetPendingResume::GuardObject
                    } else if matches!(self.ai_state, AIState::GuardRetaliating) {
                        HostJetPendingResume::GuardRetaliate
                    } else {
                        HostJetPendingResume::GuardArea
                    };
            }
            self.jet_ai.allow_interrupt_for_reload = false;
            return JetAiTickAction::ReturnToBase;
        }
        JetAiTickAction::None
    }

    fn position_jet_lockon(&mut self, now: u32) {
        let lockon_time = self.jet_lockon_time_frames();
        if self.jet_ai.untargetable_expire_frame == 0 || lockon_time == 0 {
            self.jet_ai.lockon_pos = None;
            self.jet_ai.lockon_tick_pending = false;
            return;
        }
        if now >= self.jet_ai.untargetable_expire_frame {
            self.jet_ai.untargetable_expire_frame = 0;
            self.jet_ai.lockon_pos = None;
            return;
        }
        let remaining = self.jet_ai.untargetable_expire_frame.saturating_sub(now);
        let elapsed = lockon_time.saturating_sub(remaining);
        let owner = self.get_position();
        let final_dist = self.selection_radius.max(1.0);
        let frac = remaining as f32 / lockon_time.max(1) as f32;
        let dist = final_dist + (STEALTH_FIGHTER_LOCKON_INITIAL_DIST - final_dist) * frac;
        let angle = STEALTH_FIGHTER_LOCKON_ANGLE_SPIN * frac;
        let mut pos = owner;
        pos.x += angle.cos() * dist;
        pos.z += angle.sin() * dist;
        self.jet_ai.lockon_pos = Some([pos.x, pos.y, pos.z]);

        let elapsed_prev = elapsed.saturating_sub(1);
        let elapsed_time_sum_prev = 0.5 * (elapsed_prev as f32) * (elapsed as f32);
        let elapsed_time_sum_curr = elapsed_time_sum_prev + elapsed as f32;
        let factor = STEALTH_FIGHTER_LOCKON_FREQ / lockon_time.max(1) as f32;
        let last_phase = ((factor * elapsed_time_sum_prev) as i32 & 1) != 0;
        let this_phase = ((factor * elapsed_time_sum_curr) as i32 & 1) != 0;
        if last_phase && !this_phase {
            self.jet_ai.lockon_tick_pending = true;
            self.jet_ai.lockon_hidden = false;
        } else {
            self.jet_ai.lockon_hidden = STEALTH_FIGHTER_LOCKON_BLINKY;
        }
    }

    pub fn take_jet_lockon_tick(&mut self) -> bool {
        let pending = self.jet_ai.lockon_tick_pending;
        self.jet_ai.lockon_tick_pending = false;
        pending
    }

    pub fn apply_pending_jet_resume(&mut self) {
        let resume = self.jet_ai.pending_resume;
        self.jet_ai.has_pending_command = false;
        self.jet_ai.pending_resume = HostJetPendingResume::None;
        match resume {
            HostJetPendingResume::None => {}
            HostJetPendingResume::GuardArea => {
                self.hunting = false;
                self.set_ai_state(AIState::GuardingArea);
                self.jet_ai.allow_interrupt_for_reload = true;
            }
            HostJetPendingResume::GuardObject => {
                self.hunting = false;
                self.set_ai_state(AIState::GuardingObject);
                self.jet_ai.allow_interrupt_for_reload = true;
            }
            HostJetPendingResume::Hunt => {
                self.hunting = true;
                self.auto_acquire_when_idle = true;
                self.set_ai_state(AIState::Patrolling);
                self.jet_ai.allow_interrupt_for_reload = true;
            }
            HostJetPendingResume::GuardRetaliate => {
                self.hunting = false;
                self.set_ai_state(AIState::GuardRetaliating);
                self.jet_ai.allow_interrupt_for_reload = true;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JetAiTickAction {
    None,
    ReturnToBase,
    ResumePending,
}
