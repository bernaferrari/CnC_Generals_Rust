use super::*;

impl Object {
    pub fn get_position(&self) -> Vec3 {
        self.thing.get_position()
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.thing.set_position(position);
        // Keep compatibility shadow in sync (many call sites still read `position`).
        self.position = position;
    }

    pub fn get_orientation(&self) -> f32 {
        self.thing.get_orientation()
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.thing.set_orientation(angle);
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    /// C++ ActiveBody visual condition + Drawable::reactToBodyDamageStateChange residual.

    /// C++ ProductionUpdate MODELCONDITION_CONSTRUCTION_COMPLETE residual.

    /// C++ RadarUpdate::extendRadar residual.

    /// C++ ProductionUpdate door residual:
    /// OPENING → WAITING_OPEN → WAITING_TO_CLOSE → CLOSING → idle.
    ///
    /// Retail residual timings (fail-closed vs full INI Door*Time):
    /// open 15f, wait-open 30f, wait-to-close 1f, close 15f.
    pub fn start_production_door_cycle(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        // Clear door 1 bits then set OPENING.
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
        self.model_condition_bits |= 1u128 << open_b;
        self.production_door_phase = 1;
        self.production_door_phase_end_frame = now.saturating_add(15);
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        self.record_host_production_door();
    }

    /// C++ ProductionUpdate::setHoldDoorOpen residual.
    ///
    /// When hold becomes true and the door is idle, starts OPENING residual.
    /// While hold is true, tick will not leave WAITING_OPEN / WAITING_TO_CLOSE.
    pub fn set_production_door_hold_open(&mut self, hold: bool, now: u32) {
        self.production_door_hold_open = hold;
        if hold && self.production_door_phase == 0 {
            // C++: if all door frames 0, start opening.
            self.start_production_door_cycle(now);
        }
        if !hold {
            // Allow close path to proceed from current phase on next tick.
            // If stuck in wait phases, schedule immediate advance eligibility.
            if matches!(self.production_door_phase, 2 | 3) {
                self.production_door_phase_end_frame = now;
            }
        }
        self.record_host_production_door();
    }

    /// Advance production door residual; returns true when cycle fully closed.
    /// Wave 627: apply production-door model bits after GW phase writeback.
    ///
    /// Does not advance phase schedules (GameWorld is last-writer for phase/end).
    /// Sets door-1 model condition bits for the authoritative phase.
    pub(crate) fn apply_production_door_phase_residual(&mut self, phase: u8) -> bool {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        // Clear all door-1 bits then set the phase residual.
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
        match phase {
            1 => self.model_condition_bits |= 1u128 << open_b,
            2 => self.model_condition_bits |= 1u128 << wait_b,
            3 => self.model_condition_bits |= 1u128 << wait_close_b,
            4 => self.model_condition_bits |= 1u128 << close_b,
            _ => {}
        }
        self.production_door_phase = phase;
        if phase == 0 {
            self.production_door_phase_end_frame = 0;
        }
        self.record_host_production_door();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        true
    }

    pub fn tick_production_door(&mut self, now: u32) -> bool {
        if self.production_door_phase == 0 {
            return false;
        }
        if now < self.production_door_phase_end_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        let result = match self.production_door_phase {
            1 => {
                // OPENING → WAITING_OPEN
                self.model_condition_bits &= !(1u128 << open_b);
                self.model_condition_bits |= 1u128 << wait_b;
                self.production_door_phase = 2;
                self.record_host_production_door();
                self.production_door_phase_end_frame = now.saturating_add(30);
                self.refresh_model_condition_bits();
                // Wave 486: door model bits must reach GW before model-condition writeback.
                self.record_host_model_condition();
                false
            }
            2 => {
                // C++: !m_holdOpen required to leave WAITING_OPEN.
                if self.production_door_hold_open {
                    // Keep waiting-open while held (refresh wait stamp residual).
                    self.production_door_phase_end_frame = now.saturating_add(30);
                    return false;
                }
                // WAITING_OPEN → WAITING_TO_CLOSE residual (C++ theWaitingToCloseFlags).
                self.model_condition_bits &= !(1u128 << wait_b);
                self.model_condition_bits |= 1u128 << wait_close_b;
                self.production_door_phase = 3;
                self.record_host_production_door();
                // Minimal hold before CLOSING residual (INI DoorCloseTime path).
                self.production_door_phase_end_frame = now.saturating_add(1);
                self.refresh_model_condition_bits();
                // Wave 486: door model bits must reach GW before model-condition writeback.
                self.record_host_model_condition();
                false
            }
            3 => {
                // C++: !m_holdOpen required to leave WAITING_TO_CLOSE / CLOSING path.
                if self.production_door_hold_open {
                    self.production_door_phase_end_frame = now.saturating_add(1);
                    return false;
                }
                // WAITING_TO_CLOSE → CLOSING
                self.model_condition_bits &= !(1u128 << wait_close_b);
                self.model_condition_bits |= 1u128 << close_b;
                self.production_door_phase = 4;
                self.record_host_production_door();
                self.production_door_phase_end_frame = now.saturating_add(15);
                self.refresh_model_condition_bits();
                // Wave 486: door model bits must reach GW before model-condition writeback.
                self.record_host_model_condition();
                false
            }
            4 => {
                // C++: !m_holdOpen required to finish closing.
                if self.production_door_hold_open {
                    // Snap back to waiting-open while held.
                    self.model_condition_bits &= !(1u128 << close_b);
                    self.model_condition_bits |= 1u128 << wait_b;
                    self.production_door_phase = 2;
                    self.record_host_production_door();
                    self.production_door_phase_end_frame = now.saturating_add(30);
                    self.refresh_model_condition_bits();
                    // Wave 486: door model bits must reach GW before model-condition writeback.
                    self.record_host_model_condition();
                    return false;
                }
                // CLOSING → idle
                self.model_condition_bits &= !(1u128 << close_b);
                self.production_door_phase = 0;
                self.record_host_production_door();
                self.production_door_phase_end_frame = 0;
                self.refresh_model_condition_bits();
                // Wave 486: door model bits must reach GW before model-condition writeback.
                self.record_host_model_condition();
                true
            }
            _ => {
                self.production_door_phase = 0;
                self.record_host_production_door();
                false
            }
        };
        self.record_host_model_condition();
        result
    }

    pub fn extend_radar(&mut self, done_frame: u32) {
        use crate::game_logic::host_enum_table_residual::radar_extending_model_bit;
        let bit = radar_extending_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        // Clear upgraded while extending.
        use crate::game_logic::host_enum_table_residual::radar_upgraded_model_bit;
        self.model_condition_bits &= !(1u128 << radar_upgraded_model_bit());
        self.radar_extend_done_frame = done_frame;
        self.radar_extend_complete = false;
        self.radar_active = true;
        self.record_host_radar_extend();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ RadarUpdate::update extend completion residual.
    /// Returns true when extension just completed this tick.
    /// Wave 625: apply radar-extend complete residual after GW writeback.
    ///
    /// Does not re-check done_frame (GameWorld is last-writer for complete flag).
    pub(crate) fn apply_radar_extend_complete_residual(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        self.radar_extend_complete = true;
        self.radar_extend_done_frame = 0;
        self.model_condition_bits &= !(1u128 << radar_extending_model_bit());
        self.model_condition_bits |= 1u128 << radar_upgraded_model_bit();
        self.refresh_model_condition_bits();
        self.record_host_radar_extend();
        self.record_host_model_condition();
    }

    pub fn tick_radar_extend(&mut self, current_frame: u32) -> bool {
        if self.radar_extend_done_frame == 0 || self.radar_extend_complete {
            return false;
        }
        if current_frame <= self.radar_extend_done_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        self.radar_extend_complete = true;
        self.radar_extend_done_frame = 0;
        self.model_condition_bits &= !(1u128 << radar_extending_model_bit());
        self.model_condition_bits |= 1u128 << radar_upgraded_model_bit();
        self.refresh_model_condition_bits();
        self.record_host_radar_extend();
        self.record_host_model_condition();
        true
    }

    /// C++ BuildAssistant/Dozer construction model-condition residual.
    ///
    /// - With active dozer nearby: PARTIALLY_CONSTRUCTED + ACTIVELY_BEING_CONSTRUCTED
    /// - Without dozer (waiting): AWAITING_CONSTRUCTION + PARTIALLY_CONSTRUCTED
    /// - Clears ACTIVELY_BEING when dozer leaves.

    /// C++ MODELCONDITION_ACTIVELY_CONSTRUCTING residual (dozer or factory).

    /// C++ BuildAssistant sell scaffold model residual (start of sell).
    /// C++ TechBuildingBehavior MODELCONDITION_CAPTURED residual.
    pub fn set_captured_model_condition(&mut self, captured: bool) {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        if bit == 0 {
            return;
        }
        if captured {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    pub fn has_captured_model_condition(&self) -> bool {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        bit != 0 && (self.model_condition_bits & (1u128 << bit)) != 0
    }

    pub fn apply_sell_scaffold_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, construction_complete_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        let complete_b = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << sold_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        self.model_condition_bits |= 1u128 << active_b;
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ BuildAssistant: when construction percent crosses to <= 0 during sell.
    pub fn apply_sold_model_condition(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits |= 1u128 << sold_b;
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ Object::attemptHealingFromSoleBenefactor residual.
    ///
    /// Non-stacking healers (dozer repair, ambulance, propaganda) claim exclusive
    /// heal rights for `duration` frames. Returns false if another benefactor still
    /// owns the claim.
    pub fn attempt_healing_from_sole_benefactor(
        &mut self,
        amount: f32,
        source_id: ObjectId,
        duration_frames: u32,
        now: u32,
    ) -> bool {
        if amount <= 0.0 {
            return false;
        }
        let claim_open = now > self.sole_healing_benefactor_expiration_frame
            || self.sole_healing_benefactor == Some(source_id);
        self.record_host_sole_healing();
        if !claim_open {
            return false;
        }
        self.sole_healing_benefactor = Some(source_id);
        self.record_host_sole_healing();
        self.sole_healing_benefactor_expiration_frame = now.saturating_add(duration_frames);
        self.record_host_sole_healing();
        let before = self.health.current;
        self.heal(amount);
        self.health.current > before + 0.0001 || self.health.current >= self.health.maximum - 0.01
    }

    pub fn set_actively_constructing(&mut self, active: bool) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let bit = actively_constructing_model_bit();
        if active {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    pub fn set_under_construction_model_conditions(&mut self, actively_built: bool) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let complete_b = construction_complete_model_bit();
        // Clear all construction-related bits first.
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        if actively_built {
            self.model_condition_bits |= 1u128 << active_b;
        } else {
            self.model_condition_bits |= 1u128 << await_b;
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ clear construction model conditions on finish (before CONSTRUCTION_COMPLETE).
    pub fn clear_under_construction_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ ProductionUpdate ConstructionCompleteDuration residual (default 45f / 1500ms).
    pub const CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL: u32 = 45;

    pub fn set_construction_complete_condition(&mut self) {
        self.set_construction_complete_condition_at(0);
    }

    /// Set CONSTRUCTION_COMPLETE and schedule clear after residual duration.
    /// `now==0` keeps bit sticky (legacy structure-complete path until tick).
    pub fn set_construction_complete_condition_at(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        if now > 0 {
            self.construction_complete_clear_frame =
                now.saturating_add(Self::CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL);
            self.record_host_rebuild_producer();
        } else {
            // Structure build-complete path: clear after residual duration from next tick
            // when caller supplies frame via arm helper.
            self.construction_complete_clear_frame = 0;
            self.record_host_rebuild_producer();
        }
        self.refresh_model_condition_bits();
    }

    /// Arm clear deadline for CONSTRUCTION_COMPLETE (C++ m_constructionCompleteFrame).
    pub fn arm_construction_complete_clear(&mut self, now: u32) {
        if now == 0 {
            return;
        }
        self.construction_complete_clear_frame =
            now.saturating_add(Self::CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL);
        self.record_host_rebuild_producer();
    }

    /// C++ ProductionUpdate clear CONSTRUCTION_COMPLETE after duration.
    /// Returns true when the bit was cleared this tick.
    /// Wave 626: clear CONSTRUCTION_COMPLETE model bit after GW deadline writeback.
    ///
    /// Returns true when the bit was cleared (or deadline consumed).
    pub(crate) fn apply_construction_complete_clear_residual(&mut self) -> bool {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        let had_bit = (self.model_condition_bits & (1u128 << bit)) != 0;
        let had_deadline = self.construction_complete_clear_frame > 0;
        if !had_bit && !had_deadline {
            return false;
        }
        self.model_condition_bits &= !(1u128 << bit);
        self.construction_complete_clear_frame = 0;
        self.record_host_rebuild_producer();
        self.refresh_model_condition_bits();
        true
    }

    pub fn tick_construction_complete_clear(&mut self, now: u32) -> bool {
        if self.construction_complete_clear_frame == 0 {
            return false;
        }
        if now < self.construction_complete_clear_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << bit);
        self.construction_complete_clear_frame = 0;
        self.record_host_rebuild_producer();
        self.refresh_model_condition_bits();
        true
    }

    /// Wave 623: apply BodyDamageType transition residual after GW writeback.
    ///
    /// Does not recompute state from HP (GameWorld is last-writer for the enum).
    /// Applies model bits + BoneFX/TransitionDamageFX/FXListDie peels.
    pub(crate) fn apply_body_damage_state_change_residual(
        &mut self,
        old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) {
        use crate::game_logic::host_enum_table_residual::{
            host_apply_body_damage_model_bits, HostBodyDamageType,
        };
        self.body_damage_state = state;
        crate::game_logic::host_body_damage_log::record(self.id, state.ordinal());
        if old_state != state {
            if self.bone_fx_damage.is_none()
                && crate::game_logic::host_bone_fx_damage::wants_bone_fx(&self.template_name)
            {
                self.bone_fx_damage =
                    Some(crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::default());
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                let _ = bfx.on_body_damage_state_change(&self.template_name, old_state, state);
            }
        }
        self.ensure_transition_damage_fx();
        if let Some(cfg) = self.transition_damage_fx.as_ref() {
            if let Some(ev) = crate::game_logic::host_transition_damage_fx::transition_event(
                cfg, old_state, state,
            ) {
                self.pending_transition_damage_fx.push(ev);
            }
        }
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.fire_fx_list_die();
            self.fire_create_object_die();
        }
        self.model_condition_bits =
            host_apply_body_damage_model_bits(self.model_condition_bits, state);
        self.record_host_model_condition();
    }

    pub fn refresh_model_condition_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            host_apply_body_damage_model_bits, host_calc_body_damage_state, HostBodyDamageType,
            MC_BIT_ATTACKING, MC_BIT_DISGUISED, MC_BIT_DYING, MC_BIT_MOVING,
        };
        let health = self.health.current;
        let max_h = self.health.maximum.max(0.0);
        let old_state = self.body_damage_state;
        let state = if self.status.destroyed || health <= 0.0 {
            HostBodyDamageType::Rubble
        } else {
            host_calc_body_damage_state(health, max_h)
        };
        self.body_damage_state = state;
        crate::game_logic::host_body_damage_log::record(self.id, state.ordinal());
        // C++ BoneFXDamage::onBodyDamageStateChange residual.
        if old_state != state {
            if self.bone_fx_damage.is_none()
                && crate::game_logic::host_bone_fx_damage::wants_bone_fx(&self.template_name)
            {
                self.bone_fx_damage =
                    Some(crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::default());
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                let _ = bfx.on_body_damage_state_change(&self.template_name, old_state, state);
            }
        }
        // C++ TransitionDamageFX::onBodyDamageStateChange residual.
        self.ensure_transition_damage_fx();
        if let Some(cfg) = self.transition_damage_fx.as_ref() {
            if let Some(ev) = crate::game_logic::host_transition_damage_fx::transition_event(
                cfg, old_state, state,
            ) {
                self.pending_transition_damage_fx.push(ev);
            }
        }
        // C++ FXListDie when entering rubble / destroyed.
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.fire_fx_list_die();
            self.fire_create_object_die();
        }
        let mut bits = host_apply_body_damage_model_bits(self.model_condition_bits, state);
        // Motion / combat residual bits from ObjectStatus.
        if self.status.moving {
            bits |= 1u128 << MC_BIT_MOVING;
        } else {
            bits &= !(1u128 << MC_BIT_MOVING);
        }
        if self.status.attacking {
            bits |= 1u128 << MC_BIT_ATTACKING;
        } else {
            bits &= !(1u128 << MC_BIT_ATTACKING);
        }
        if self.status.destroyed {
            bits |= 1u128 << MC_BIT_DYING;
        } else {
            bits &= !(1u128 << MC_BIT_DYING);
        }
        if self.status.disguised {
            bits |= 1u128 << MC_BIT_DISGUISED;
        } else {
            bits &= !(1u128 << MC_BIT_DISGUISED);
        }
        // C++ Physics stun model conditions residual.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
        bits &= !(1u128 << MC_BIT_STUNNED);
        if self.shock_stun_frames > 15 {
            bits |= 1u128 << MC_BIT_STUNNED_FLAILING;
        } else if self.shock_stun_frames > 0 {
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // FREEFALL residual: airborne while stunned (C++ IS_IN_FREEFALL path).
        use crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
        bits &= !(1u128 << MC_BIT_FREEFALL);
        let airborne = self.get_position().y > 0.05 || self.movement.velocity.y > 1.0;
        if airborne && self.shock_stun_frames > 0 && self.shock_was_airborne {
            bits |= 1u128 << MC_BIT_FREEFALL;
        }
        // After first ground contact, prefer STUNNED over FLAILING even if frames high.
        if self.shock_grounded_once && self.shock_stun_frames > 0 {
            use crate::game_logic::host_enum_table_residual::{
                MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
            };
            bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // Radar extend residual sticks across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        let had_radar_ext =
            (self.model_condition_bits & (1u128 << radar_extending_model_bit())) != 0;
        let had_radar_upg =
            (self.model_condition_bits & (1u128 << radar_upgraded_model_bit())) != 0;
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        let had_door_open =
            (self.model_condition_bits & (1u128 << door_1_opening_model_bit())) != 0;
        let had_door_wait =
            (self.model_condition_bits & (1u128 << door_1_waiting_open_model_bit())) != 0;
        let had_door_wait_close =
            (self.model_condition_bits & (1u128 << door_1_waiting_to_close_model_bit())) != 0;
        let had_door_close =
            (self.model_condition_bits & (1u128 << door_1_closing_model_bit())) != 0;

        // SPLATTED residual sticks after fatal falling damage.
        use crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
        let had_splat = (self.model_condition_bits & (1u128 << MC_BIT_SPLATTED)) != 0;
        if had_splat
            || (self.status.destroyed
                && self.status.death_type
                    == crate::game_logic::host_usa_pilot::HostDeathType::Splatted)
        {
            bits |= 1u128 << MC_BIT_SPLATTED;
            crate::game_logic::host_death_type_log::record(
                self.id,
                self.status.death_type.ordinal(),
            );
        }
        if had_radar_ext {
            bits |= 1u128 << radar_extending_model_bit();
        }
        if had_radar_upg {
            bits |= 1u128 << radar_upgraded_model_bit();
        }
        if had_door_open {
            bits |= 1u128 << door_1_opening_model_bit();
        }
        if had_door_wait {
            bits |= 1u128 << door_1_waiting_open_model_bit();
        }
        if had_door_wait_close {
            bits |= 1u128 << door_1_waiting_to_close_model_bit();
        }
        if had_door_close {
            bits |= 1u128 << door_1_closing_model_bit();
        }
        // Construction residual bits stick across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let had_cc =
            (self.model_condition_bits & (1u128 << construction_complete_model_bit())) != 0;
        let had_await =
            (self.model_condition_bits & (1u128 << awaiting_construction_model_bit())) != 0;
        let had_partial =
            (self.model_condition_bits & (1u128 << partially_constructed_model_bit())) != 0;
        let had_active =
            (self.model_condition_bits & (1u128 << actively_being_constructed_model_bit())) != 0;
        if had_cc {
            bits |= 1u128 << construction_complete_model_bit();
        }
        if had_await {
            bits |= 1u128 << awaiting_construction_model_bit();
        }
        if had_partial {
            bits |= 1u128 << partially_constructed_model_bit();
        }
        if had_active {
            bits |= 1u128 << actively_being_constructed_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let had_ac =
            (self.model_condition_bits & (1u128 << actively_constructing_model_bit())) != 0;
        if had_ac {
            bits |= 1u128 << actively_constructing_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::sold_model_bit;
        let had_sold_mc = (self.model_condition_bits & (1u128 << sold_model_bit())) != 0;
        if had_sold_mc {
            bits |= 1u128 << sold_model_bit();
        }
        // Wave 487: preserve combat/presentation bits refresh rebuild does not recompute.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_C, MC_BIT_FIRING_A, MC_BIT_FIRING_B, MC_BIT_FIRING_C,
            MC_BIT_PREATTACK_A, MC_BIT_PREATTACK_B, MC_BIT_PREATTACK_C, MC_BIT_RELOADING_A,
            MC_BIT_RELOADING_B, MC_BIT_RELOADING_C,
        };
        const WEAPON_MC_PRESERVE: [u32; 12] = [
            MC_BIT_PREATTACK_A,
            MC_BIT_FIRING_A,
            MC_BIT_BETWEEN_FIRING_SHOTS_A,
            MC_BIT_RELOADING_A,
            MC_BIT_PREATTACK_B,
            MC_BIT_FIRING_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_RELOADING_B,
            MC_BIT_PREATTACK_C,
            MC_BIT_FIRING_C,
            MC_BIT_BETWEEN_FIRING_SHOTS_C,
            MC_BIT_RELOADING_C,
        ];
        for b in WEAPON_MC_PRESERVE {
            if (self.model_condition_bits & (1u128 << b)) != 0 {
                bits |= 1u128 << b;
            }
        }
        if let Some(bit) =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("PRONE")
        {
            if (self.model_condition_bits & (1u128 << bit)) != 0 {
                bits |= 1u128 << bit;
            }
        }
        {
            use crate::game_logic::host_neutron_missile_slow_death::{
                MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
            };
            if (self.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED)) != 0 {
                bits |= 1u128 << MC_BIT_FRONTCRUSHED;
            }
            if (self.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED)) != 0 {
                bits |= 1u128 << MC_BIT_BACKCRUSHED;
            }
        }
        self.model_condition_bits = bits;
        self.record_host_model_condition();
    }
}
