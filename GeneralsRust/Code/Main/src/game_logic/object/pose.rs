use super::*;

impl Object {
    pub fn get_position(&self) -> Vec3 {
        self.thing.get_position()
    }

    pub fn set_position(&mut self, position: Vec3) {
        let old_ix = self.position.x as i32;
        let old_iz = self.position.z as i32;
        let old_cell = crate::game_logic::partition_manager::PartitionManager::cell_coords(
            self.position.x,
            self.position.z,
        );
        self.thing.set_position(position);
        // Keep compatibility shadow in sync (many call sites still read `position`).
        self.position = position;
        let new_cell = crate::game_logic::partition_manager::PartitionManager::cell_coords(
            position.x, position.z,
        );
        if old_cell != new_cell {
            self.restamp_partition_value_threat();
        }
        // C++ Object.cpp:2580-2583: integer XY change notifies W3DTreeBuffer::unitMoved.
        if old_ix != position.x as i32 || old_iz != position.z as i32 {
            self.notify_terrain_trees_on_unit_move();
            let skip = self.is_kind_of(KindOf::Projectile);
            let team = if self.team_instance_name.is_empty() {
                None
            } else {
                Some(self.team_instance_name.as_str())
            };
            gamelogic::scripting::update_host_object_trigger_flags(
                self.id.0,
                position.x,
                position.z,
                gamelogic::system::game_logic::current_frame(),
                skip,
                team,
            );
        }
    }

    /// C++ `TheGameClient->notifyTerrainObjectMoved` → `W3DTreeBuffer::unitMoved`.
    pub fn notify_terrain_trees_on_unit_move(&self) {
        if self.is_kind_of(KindOf::Projectile) || self.is_kind_of(KindOf::Immobile) {
            return;
        }
        if !self.is_kind_of(KindOf::Infantry) && !self.is_kind_of(KindOf::Vehicle) {
            return;
        }
        #[cfg(feature = "game_client")]
        {
            let pos = self.get_position();
            let angle = self.get_orientation();
            let radius = self.selection_radius.max(1.0);
            let unit = game_client::terrain::TreeCollisionUnit {
                object_id: self.id.0,
                // Tree buffer stores C++ XY-horizontal; host pose is Y-up.
                position: Vec3::new(pos.x, pos.z, pos.y),
                direction_2d: glam::Vec2::new(angle.cos(), -angle.sin()),
                major_radius: radius,
                minor_radius: radius,
                geometry_type: game_client::terrain::TreeGeometryType::Cylinder,
                crusher_level: i32::from(self.crusher_level),
                immobile: false,
            };
            game_client::terrain::notify_terrain_unit_moved(
                unit,
                gamelogic::system::game_logic::current_frame(),
            );
        }
    }

    /// C++ Object::addValue/addThreat via PartitionManager (Object.cpp:4807-4883).
    pub fn stamp_partition_value_threat(&mut self) {
        if self.status.under_construction || !self.is_alive() {
            self.unstamp_partition_value_threat();
            return;
        }
        if self.shroud_clearing_range <= 0.0 {
            self.unstamp_partition_value_threat();
            return;
        }
        let Some(player_id) = self.owner_player_id else {
            self.unstamp_partition_value_threat();
            return;
        };
        self.unstamp_partition_value_threat();
        let stamp = crate::game_logic::partition_manager::HostPartitionAffectStamp {
            x: self.position.x,
            z: self.position.z,
            range: self.vision_range.max(1.0),
            value: self.partition_cash_value,
            threat: self.partition_threat_value,
            mask: 1u32 << player_id.min(15),
        };
        stamp.apply(true);
        self.partition_last_affect = Some(stamp);
    }

    pub fn unstamp_partition_value_threat(&mut self) {
        if let Some(stamp) = self.partition_last_affect.take() {
            stamp.apply(false);
        }
    }

    /// C++ Object::handlePartitionCellMaintenance value+threat half
    /// (Object.cpp:4771-4776 handleValueMap + handleThreatMap): unstamp then restamp
    /// with the current controlling-player mask.
    pub fn handle_partition_cell_maintenance(&mut self) {
        self.stamp_partition_value_threat();
    }

    fn restamp_partition_value_threat(&mut self) {
        if self.partition_last_affect.is_some() || self.partition_cash_value > 0 {
            self.stamp_partition_value_threat();
        }
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

    /// C++ `Object::getHealthBoxPosition` (`Object.cpp:3346-3356`).
    /// Host pose is Y-up: C++ world Z maps to `position.y`.
    pub fn get_health_box_position(&self) -> Vec3 {
        let pos = self.get_position();
        let geom = &self.thing.template.geometry_info;
        let max_h = if geom.authored {
            geom.max_height_above_position()
        } else {
            self.selection_radius.max(1.0)
        };
        let mut y = pos.y + max_h + 10.0 + self.health_box_offset[1];
        if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(&self.template_name) {
            y += 20.0;
        }
        Vec3::new(
            pos.x + self.health_box_offset[0],
            y,
            pos.z + self.health_box_offset[2],
        )
    }

    /// C++ `Object::getHealthBoxDimensions` (`Object.cpp:3402-3413`).
    /// Returns `(height, width)`. IGNORED_IN_GUI → `(0, 0)`.
    pub fn get_health_box_dimensions(&self) -> (f32, f32) {
        if self.is_kind_of(KindOf::IgnoredInGui) {
            return (0.0, 0.0);
        }
        let geom = &self.thing.template.geometry_info;
        let (major, minor) = if geom.authored {
            (geom.major_radius, geom.minor_radius)
        } else {
            let r = self.selection_radius.max(1.0);
            (r, r)
        };
        let size = (major + minor).min(150.0).max(20.0);
        (3.0, (size * 2.0).max(20.0))
    }

    /// Presentation overlay: C++ Z-up height of the health bar above ground pose.
    pub fn health_box_world_z_offset(&self) -> f32 {
        let geom = &self.thing.template.geometry_info;
        let max_h = if geom.authored {
            geom.max_height_above_position()
        } else {
            self.selection_radius.max(1.0)
        };
        let mut z = max_h + 10.0 + self.health_box_offset[1];
        if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(&self.template_name) {
            z += 20.0;
        }
        z
    }

    /// C++ ActiveBody visual condition + Drawable::reactToBodyDamageStateChange residual.

    /// C++ ProductionUpdate MODELCONDITION_CONSTRUCTION_COMPLETE residual.

    /// C++ RadarUpdate::extendRadar residual.

    /// C++ ProductionUpdate door residual:
    /// OPENING → WAITING_OPEN → CLOSING → idle.
    /// `theWaitingToCloseFlags` is never set (ProductionUpdate.cpp:513-582).
    ///
    /// Phase durations come from INI DoorOpeningTime / DoorWaitOpenTime /
    /// DoorCloseTime (ProductionUpdate.cpp:113-115, updateDoors:528/548/568).
    /// Completing a unit opens only the reserved ExitDoorType (DOOR_1..4).
    pub fn start_production_door_cycle(&mut self, now: u32) {
        // C++ ProductionUpdate.cpp:746-754: start OPENING only when the reserved
        // door is fully closed. pick_idle must not open a sibling hangar while
        // the reserved door is already OPENING (that would reset spawn gating).
        if self.production_door_spawn_phase() != 0 {
            return;
        }
        let door = self
            .pick_idle_production_door()
            .unwrap_or(self.production_door_active_index as usize);
        self.start_production_door_cycle_at(now, door);
    }

    /// Start OPENING on a specific hangar / factory door (0=DOOR_1).
    ///
    /// C++ ProductionUpdate.cpp:746-773: a fully-closed door starts OPENING
    /// once. Already-OPENING leaves `m_doorOpenedFrame` (DoorOpeningTime)
    /// intact so the door can reach WAITING_OPEN and release the unit.
    pub fn start_production_door_cycle_at(&mut self, now: u32, door: usize) {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let door = door.min(3);
        let current = if self.production_door_phases[door] != 0 {
            self.production_door_phases[door]
        } else if door == 0 {
            self.production_door_phase
        } else {
            0
        };
        if current != 0 {
            return;
        }
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, 1);
        self.production_door_active_index = door as u8;
        self.production_door_phases[door] = 1;
        self.production_door_phase_end_frames[door] =
            now.saturating_add(producer_door_phase_duration(&self.template_name, 1));
        self.sync_primary_production_door();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        self.record_host_production_door();
    }

    /// C++ ProductionUpdate.cpp:762-776: a completed unit whose reserved door
    /// is already CLOSING (`m_doorClosedFrame != 0`) pops immediately to
    /// WAITING_OPEN so spawn proceeds this frame.
    pub fn pop_closing_production_door_to_waiting_open(&mut self, now: u32) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let door = (self.production_door_active_index as usize).min(3);
        let phase = self.production_door_spawn_phase();
        if phase != 4 && phase != 3 {
            return false;
        }
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, 2);
        self.production_door_phases[door] = 2;
        self.production_door_phase_end_frames[door] =
            now.saturating_add(producer_door_phase_duration(&self.template_name, 2));
        self.sync_primary_production_door();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        self.record_host_production_door();
        true
    }

    /// C++ ProductionUpdate::setHoldDoorOpen residual.
    ///
    /// Door 0 wrapper for factory / save callers. Hangar stalls use
    /// `set_production_door_hold_open_at`.
    pub fn set_production_door_hold_open(&mut self, hold: bool, now: u32) {
        self.set_production_door_hold_open_at(0, hold, now);
    }

    /// C++ `ProductionUpdate::setHoldDoorOpen(exitDoor, holdIt)` — one stall.
    /// Starts OPENING on that door only when it is fully closed.
    pub fn set_production_door_hold_open_at(&mut self, door: usize, hold: bool, now: u32) {
        let door = door.min(3);
        self.production_door_hold_opens[door] = hold;
        if door == 0 {
            self.production_door_hold_open = hold;
        }
        if hold {
            let phase = if self.production_door_phases[door] != 0 {
                self.production_door_phases[door]
            } else if door == 0 {
                self.production_door_phase
            } else {
                0
            };
            if phase == 0 {
                self.start_production_door_cycle_at(now, door);
            }
        } else if matches!(self.production_door_phases[door], 2 | 4)
            || (door == 0 && matches!(self.production_door_phase, 2 | 4))
        {
            self.production_door_phase_end_frames[door] = now;
            if door == 0 {
                self.production_door_phase_end_frame = now;
            }
        }
        self.sync_primary_production_door();
        self.record_host_production_door();
    }

    /// C++ DoorInfo::m_holdOpen for this ExitDoorType index.
    pub fn production_door_is_held(&self, door: usize) -> bool {
        let door = door.min(3);
        self.production_door_hold_opens[door] || (door == 0 && self.production_door_hold_open)
    }

    /// Advance production door residual; returns true when every door fully closed.
    /// Wave 627: apply production-door model bits after GW phase writeback.
    ///
    /// Does not advance phase schedules (GameWorld is last-writer for phase/end).
    /// Phase 3 (legacy WAITING_TO_CLOSE writeback) maps to CLOSING — C++ never plays it.
    pub(crate) fn apply_production_door_phase_residual(&mut self, phase: u8) -> bool {
        let door = (self.production_door_active_index as usize).min(3);
        let phase = if phase == 3 { 4 } else { phase };
        self.clear_production_door_bits(door);
        self.set_production_door_phase_bits(door, phase);
        self.production_door_phases[door] = phase;
        if phase == 0 {
            self.production_door_phase_end_frames[door] = 0;
        }
        self.sync_primary_production_door();
        self.record_host_production_door();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        true
    }

    pub fn tick_production_door(&mut self, now: u32) -> bool {
        self.ensure_primary_door_array_sync();
        let count = self.production_door_count();
        let mut closed_any = false;
        let mut advanced = false;
        for i in 0..count {
            if self.production_door_phases[i] == 0 {
                continue;
            }
            if now < self.production_door_phase_end_frames[i] {
                continue;
            }
            advanced = true;
            if self.tick_one_production_door(i, now) {
                closed_any = true;
            }
        }
        self.sync_primary_production_door();
        if advanced {
            self.record_host_model_condition();
        }
        closed_any && self.production_door_phases.iter().all(|&p| p == 0)
    }

    /// Door count from INI `NumDoorAnimations`, at least 1 so existing
    /// single-door producers keep the DOOR_1 cycle.
    pub fn production_door_count(&self) -> usize {
        use crate::game_logic::host_production_buildable_command_residual::producer_num_door_animations;
        let n = producer_num_door_animations(&self.template_name);
        if n <= 0 { 1 } else { (n as usize).min(4) }
    }

    /// First idle hangar among authored doors.
    pub fn pick_idle_production_door(&self) -> Option<usize> {
        let count = self.production_door_count();
        (0..count).find(|&i| self.production_door_phases[i] == 0)
    }

    /// Phase used for spawn gating: the reserved/active door, not a shared bank.
    pub fn production_door_spawn_phase(&self) -> u8 {
        let i = (self.production_door_active_index as usize).min(3);
        let p = self.production_door_phases[i];
        if p != 0 {
            p
        } else {
            self.production_door_phase
        }
    }

    fn ensure_primary_door_array_sync(&mut self) {
        if self.production_door_phases[0] == 0 && self.production_door_phase != 0 {
            self.production_door_phases[0] = self.production_door_phase;
            self.production_door_phase_end_frames[0] = self.production_door_phase_end_frame;
        }
        if !self.production_door_hold_opens[0] && self.production_door_hold_open {
            self.production_door_hold_opens[0] = true;
        }
    }

    fn sync_primary_production_door(&mut self) {
        self.production_door_phase = self.production_door_phases[0];
        self.production_door_phase_end_frame = self.production_door_phase_end_frames[0];
        self.production_door_hold_open = self.production_door_hold_opens[0];
    }

    fn tick_one_production_door(&mut self, door: usize, now: u32) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
        let name = self.template_name.clone();
        match self.production_door_phases[door] {
            1 => {
                self.clear_production_door_bits(door);
                self.set_production_door_phase_bits(door, 2);
                self.production_door_phases[door] = 2;
                self.production_door_phase_end_frames[door] =
                    now.saturating_add(producer_door_phase_duration(name.as_str(), 2));
                self.record_host_production_door();
                self.refresh_model_condition_bits();
                self.record_host_model_condition();
                false
            }
            2 => {
                if self.production_door_is_held(door) {
                    self.production_door_phase_end_frames[door] =
                        now.saturating_add(producer_door_phase_duration(name.as_str(), 2));
                    return false;
                }
                // C++ updateDoors: WAITING_OPEN → CLOSING. Never WAITING_TO_CLOSE.
                self.clear_production_door_bits(door);
                self.set_production_door_phase_bits(door, 4);
                self.production_door_phases[door] = 4;
                self.production_door_phase_end_frames[door] =
                    now.saturating_add(producer_door_phase_duration(name.as_str(), 4));
                self.record_host_production_door();
                self.refresh_model_condition_bits();
                self.record_host_model_condition();
                false
            }
            3 | 4 => {
                if self.production_door_is_held(door) {
                    self.clear_production_door_bits(door);
                    self.set_production_door_phase_bits(door, 2);
                    self.production_door_phases[door] = 2;
                    self.production_door_phase_end_frames[door] =
                        now.saturating_add(producer_door_phase_duration(name.as_str(), 2));
                    self.record_host_production_door();
                    self.refresh_model_condition_bits();
                    self.record_host_model_condition();
                    return false;
                }
                self.clear_production_door_bits(door);
                self.production_door_phases[door] = 0;
                self.production_door_phase_end_frames[door] = 0;
                self.record_host_production_door();
                self.refresh_model_condition_bits();
                self.record_host_model_condition();
                true
            }
            _ => {
                self.production_door_phases[door] = 0;
                self.production_door_phase_end_frames[door] = 0;
                self.record_host_production_door();
                false
            }
        }
    }

    fn production_door_bank_bits(door: usize) -> (u32, u32, u32, u32) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit, door_2_closing_model_bit, door_2_opening_model_bit,
            door_2_waiting_open_model_bit, door_2_waiting_to_close_model_bit,
            door_3_closing_model_bit, door_3_opening_model_bit, door_3_waiting_open_model_bit,
            door_3_waiting_to_close_model_bit, door_4_closing_model_bit, door_4_opening_model_bit,
            door_4_waiting_open_model_bit, door_4_waiting_to_close_model_bit,
        };
        match door {
            1 => (
                door_2_opening_model_bit(),
                door_2_waiting_open_model_bit(),
                door_2_waiting_to_close_model_bit(),
                door_2_closing_model_bit(),
            ),
            2 => (
                door_3_opening_model_bit(),
                door_3_waiting_open_model_bit(),
                door_3_waiting_to_close_model_bit(),
                door_3_closing_model_bit(),
            ),
            3 => (
                door_4_opening_model_bit(),
                door_4_waiting_open_model_bit(),
                door_4_waiting_to_close_model_bit(),
                door_4_closing_model_bit(),
            ),
            _ => (
                door_1_opening_model_bit(),
                door_1_waiting_open_model_bit(),
                door_1_waiting_to_close_model_bit(),
                door_1_closing_model_bit(),
            ),
        }
    }

    fn clear_production_door_bits(&mut self, door: usize) {
        let (open_b, wait_b, wait_close_b, close_b) = Self::production_door_bank_bits(door);
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
    }

    fn set_production_door_phase_bits(&mut self, door: usize, phase: u8) {
        let (open_b, wait_b, _wait_close_b, close_b) = Self::production_door_bank_bits(door);
        match phase {
            1 => self.model_condition_bits |= 1u128 << open_b,
            2 => self.model_condition_bits |= 1u128 << wait_b,
            3 | 4 => self.model_condition_bits |= 1u128 << close_b,
            _ => {}
        }
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

    /// Legacy 45f residual name. Live deadline uses INI ConstructionCompleteDuration
    /// (C++ module default 0 → one-frame flash).
    pub const CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL: u32 = 45;

    fn construction_complete_duration_frames(&self) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::producer_construction_complete_duration_frames;
        let authored = producer_construction_complete_duration_frames(&self.template_name);
        // Duration 0 still flashes one frame (C++ now - start > 0 on the next update).
        authored.max(1)
    }

    pub fn set_construction_complete_condition(&mut self) {
        self.set_construction_complete_condition_at(0);
    }

    /// Set CONSTRUCTION_COMPLETE and schedule clear after INI duration.
    /// `now==0` keeps bit sticky (legacy structure-complete path until tick).
    pub fn set_construction_complete_condition_at(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        if now > 0 {
            self.construction_complete_clear_frame =
                now.saturating_add(self.construction_complete_duration_frames());
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
            now.saturating_add(self.construction_complete_duration_frames());
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

    fn queue_transition_damage_fx_event(
        &mut self,
        mut ev: crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
    ) {
        crate::game_logic::host_transition_damage_fx::take_played_transition_event_fx_ocl(
            &mut ev,
            self.id.0,
            self.get_position(),
            self.get_orientation(),
            self.thing.template.get_model_name(),
            self.thing.template.asset_scale,
        );
        self.pending_transition_damage_fx.push(ev);
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
            HostBodyDamageType, host_apply_body_damage_model_bits,
        };
        self.body_damage_state = state;
        if old_state != state {
            crate::game_logic::host_move_ambient_audio::record_ambient_restart(self.id);
        }
        crate::game_logic::host_body_damage_log::record(self.id, state.ordinal());
        // C++ ActiveBody.cpp:1219 — no damaged-art visuals while building.
        let show_health_visuals = !self.status.under_construction || self.status.destroyed;
        if old_state != state && show_health_visuals {
            if self.bone_fx_damage.is_none() {
                self.bone_fx_damage =
                    crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::from_template(
                        &self.template_name,
                    );
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                bfx.stamp_last_damage_type(self.last_damage_info_type);
                let _ = bfx.on_body_damage_state_change(&self.template_name, old_state, state);
            }
        }
        if show_health_visuals {
            self.ensure_transition_damage_fx();
            if let Some(cfg) = self.transition_damage_fx.as_ref() {
                if let Some(ev) =
                    crate::game_logic::host_transition_damage_fx::on_body_damage_state_change_for_damage(
                        cfg,
                        old_state,
                        state,
                        self.last_damage_info_type.map(|d| d.to_store()),
                    )
                {
                    self.queue_transition_damage_fx_event(ev);
                }
            }
        }

        // C++ ActiveBody::setCorrectDamageState rubble (ActiveBody.cpp:189-208)
        // is independent of UNDER_CONSTRUCTION visual skip.
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.apply_structure_rubble_collision_effects();
        }
        if show_health_visuals
            && matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.fire_fx_list_die();
            self.fire_create_object_die();
            if !matches!(
                self.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed
            ) {
                self.fire_crush_die();
            }
        }
        let visual_state = if show_health_visuals {
            state
        } else {
            HostBodyDamageType::Pristine
        };
        self.model_condition_bits =
            host_apply_body_damage_model_bits(self.model_condition_bits, visual_state);
        self.record_host_model_condition();
        self.sync_overlord_addon_body_damage();
    }

    pub fn refresh_model_condition_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DISGUISED, MC_BIT_DYING, MC_BIT_MOVING,
            host_apply_body_damage_model_bits, host_calc_body_damage_state,
        };
        let health = self.health.current;
        let max_h = self.health.maximum.max(0.0);
        let old_state = self.body_damage_state;
        let state = if self.status.destroyed || health <= 0.0 {
            HostBodyDamageType::Rubble
        } else {
            // GameData UnitDamagedThresh 0.7 / ReallyDamaged 0.35 (ActiveBody.cpp:88).
            host_calc_body_damage_state(health, max_h)
        };
        self.body_damage_state = state;
        crate::game_logic::host_body_damage_log::record(self.id, state.ordinal());
        // C++ ActiveBody::setCorrectDamageState rubble (ActiveBody.cpp:189-208).
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.apply_structure_rubble_collision_effects();
        }

        // C++ GarrisonContain::onBodyDamageStateChange (GarrisonContain.cpp:1724-1732).
        if old_state != state
            && matches!(state, HostBodyDamageType::ReallyDamaged)
            && self.is_garrison_contain()
            && !self.is_kind_of(crate::game_logic::KindOf::GarrisonableUntilDestroyed)
            && !self.contained_units().is_empty()
        {
            self.status.pending_garrison_really_damaged_eject = true;
        }
        // C++ ActiveBody.cpp:1219 — skip evaluateVisualCondition while
        // OBJECT_STATUS_UNDER_CONSTRUCTION (no damaged-art health visuals).
        let show_health_visuals = !self.status.under_construction || self.status.destroyed;
        // C++ BoneFXDamage::onBodyDamageStateChange residual.
        if old_state != state && show_health_visuals {
            if self.bone_fx_damage.is_none() {
                self.bone_fx_damage =
                    crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::from_template(
                        &self.template_name,
                    );
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                bfx.stamp_last_damage_type(self.last_damage_info_type);
                let _ = bfx.on_body_damage_state_change(&self.template_name, old_state, state);
            }
        }
        if show_health_visuals {
            self.ensure_transition_damage_fx();
            if let Some(cfg) = self.transition_damage_fx.as_ref() {
                if let Some(ev) =
                    crate::game_logic::host_transition_damage_fx::on_body_damage_state_change_for_damage(
                        cfg,
                        old_state,
                        state,
                        self.last_damage_info_type.map(|d| d.to_store()),
                    )
                {
                    self.queue_transition_damage_fx_event(ev);
                }
            }
        }

        // C++ FXListDie when entering rubble / destroyed.
        if show_health_visuals
            && matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.fire_fx_list_die();
            self.fire_create_object_die();
            if !matches!(
                self.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed
            ) {
                self.fire_crush_die();
            }
        }
        let visual_state = if show_health_visuals {
            state
        } else {
            HostBodyDamageType::Pristine
        };
        let mut bits = host_apply_body_damage_model_bits(self.model_condition_bits, visual_state);
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
        for door in 0..4 {
            let (open_b, wait_b, wait_close_b, close_b) = Self::production_door_bank_bits(door);
            let had_open = (self.model_condition_bits & (1u128 << open_b)) != 0;
            let had_wait = (self.model_condition_bits & (1u128 << wait_b)) != 0;
            let had_close = (self.model_condition_bits & (1u128 << close_b)) != 0;
            // C++ ProductionUpdate never sets WAITING_TO_CLOSE; do not restore it.
            let _ = wait_close_b;
            if had_open {
                bits |= 1u128 << open_b;
            }
            if had_wait {
                bits |= 1u128 << wait_b;
            }
            if had_close {
                bits |= 1u128 << close_b;
            }
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
        {
            use crate::game_logic::host_enum_table_residual::{
                armorset_crateupgrade_one_model_bit, armorset_crateupgrade_two_model_bit,
            };
            let one = armorset_crateupgrade_one_model_bit();
            let two = armorset_crateupgrade_two_model_bit();
            bits &= !(1u128 << one);
            bits &= !(1u128 << two);
            match self.armor_crate_upgrade {
                1 => bits |= 1u128 << one,
                n if n >= 2 => bits |= 1u128 << two,
                _ => {}
            }
        }
        // C++ FlammableUpdate tryToIgnite / update: MODELCONDITION_AFLAME / SMOLDERING.
        {
            use crate::game_logic::host_enum_table_residual::{
                aflame_model_bit, smoldering_model_bit,
            };
            let aflame_b = aflame_model_bit();
            let smolder_b = smoldering_model_bit();
            bits &= !(1u128 << aflame_b);
            bits &= !(1u128 << smolder_b);
            let live_aflame = self.has_object_status_bit("AFLAME")
                || self
                    .fire_spread
                    .as_ref()
                    .map(|f| f.is_aflame())
                    .unwrap_or(false);
            let live_smolder = self
                .fire_spread
                .as_ref()
                .map(|f| f.smoldering)
                .unwrap_or(false)
                || (self.has_object_status_bit("BURNED") && live_aflame)
                || (self.has_object_status_bit("BURNED") && !live_aflame);
            if live_aflame {
                bits |= 1u128 << aflame_b;
            }
            if live_smolder {
                bits |= 1u128 << smolder_b;
            }
        }
        // C++ W3DModelDraw::updateDrawModuleSupplyStatus — CARRYING while boxes > 0.
        {
            use crate::game_logic::host_enum_table_residual::carrying_model_bit;
            let carry_b = carrying_model_bit();
            bits &= !(1u128 << carry_b);
            if self.stored_resources.supplies > 0 {
                bits |= 1u128 << carry_b;
            }
        }
        // C++ MODELCONDITION_PANICKING from chooseLocomotorSet(PANIC) / AIPanicState.
        // C++ MODELCONDITION_OVER_WATER from Locomotor hover flag.
        {
            use crate::game_logic::host_enum_table_residual::over_water_model_bit;
            let bit = over_water_model_bit();
            bits &= !(1u128 << bit);
            if self.over_water {
                bits |= 1u128 << bit;
            }
        }
        {
            use crate::game_logic::host_enum_table_residual::panicking_model_bit;
            let panic_b = panicking_model_bit();
            bits &= !(1u128 << panic_b);
            if self.is_panicking {
                bits |= 1u128 << panic_b;
            }
        }
        if self.model_condition_bits != bits {
            self.visual_draw_state_revision =
                self.visual_draw_state_revision.saturating_add(1).max(1);
        }
        self.model_condition_bits = bits;
        self.sync_overlord_addon_body_damage();
        self.record_host_model_condition();
    }
}
