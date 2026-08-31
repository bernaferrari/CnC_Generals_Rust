use super::*;

impl Object {
    pub fn select(&mut self) {
        if self.is_selectable() {
            self.selected = true;
            self.status.selected = true;
            crate::game_logic::host_status_log::record_selected(self.id, true);
        }
    }

    pub fn deselect(&mut self) {
        self.selected = false;
        self.status.selected = false;
        crate::game_logic::host_status_log::record_selected(self.id, false);
    }

    /// C++ `Drawable::m_hidden || m_hiddenByStealth`.
    #[inline]
    pub fn drawable_is_effectively_hidden(&self) -> bool {
        gamelogic::object::draw::leftover_hidden_status_deselects(
            self.drawable_hidden,
            self.camo_stealth_look == 5,
        )
    }

    /// C++ `Drawable::setDrawableHidden` → `updateHiddenStatus`.
    pub fn set_drawable_hidden(&mut self, hidden: bool) {
        if self.drawable_hidden == hidden {
            return;
        }
        self.drawable_hidden = hidden;
        self.update_drawable_hidden_status();
    }

    /// C++ `Drawable::updateHiddenStatus` — deselect when hidden/stealth-invisible.
    pub fn update_drawable_hidden_status(&mut self) {
        if self.drawable_is_effectively_hidden() && (self.selected || self.status.selected) {
            self.deselect();
        }
    }

    /// Host combat residual: mark attacking and log for GameWorld status channel.
    pub fn set_status_attacking(&mut self, attacking: bool) {
        self.status.attacking = attacking;
        crate::game_logic::host_status_log::record_attacking(self.id, attacking);
    }

    /// Host weapon fire residual + status channel log.
    pub fn set_status_firing_weapon(&mut self, firing: bool) {
        self.status.is_firing_weapon = firing;
        if firing {
            self.blow_defector_cover();
        }
        crate::game_logic::host_status_log::record_firing(self.id, firing);
    }

    /// Host weapon aim residual + status channel log.
    pub fn set_status_aiming_weapon(&mut self, aiming: bool) {
        self.status.is_aiming_weapon = aiming;
        crate::game_logic::host_status_log::record_aiming(self.id, aiming);
    }

    /// Host stealth residual + status channel log.
    pub fn set_status_stealthed(&mut self, stealthed: bool) {
        self.status.stealthed = stealthed;
        crate::game_logic::host_status_log::record_stealthed(self.id, stealthed);
    }

    /// Host detection residual + status channel log.
    pub fn set_status_detected(&mut self, detected: bool) {
        self.status.detected = detected;
        crate::game_logic::host_status_log::record_detected(self.id, detected);
    }

    /// Host EMP disable residual + status channel log.
    pub fn set_status_disabled_emp(&mut self, disabled: bool) {
        self.status.disabled_emp = disabled;
        crate::game_logic::host_status_log::record_disabled_emp(self.id, disabled);
    }

    /// Host weapon jam residual + status channel log.
    pub fn set_status_weapons_jammed(&mut self, jammed: bool) {
        self.status.weapons_jammed = jammed;
        crate::game_logic::host_status_log::record_weapons_jammed(self.id, jammed);
    }

    pub fn set_status_moving(&mut self, moving: bool) {
        self.status.moving = moving;
        crate::game_logic::host_status_log::record_moving(self.id, moving);
    }

    /// C++ setCompletedWaypoint path labels while following a script/player path.
    pub fn stamp_pending_waypoint_labels(
        &mut self,
        labels: impl IntoIterator<Item = impl Into<String>>,
    ) {
        let mut out = Vec::new();
        for label in labels {
            let s = label.into();
            if s.is_empty() {
                continue;
            }
            if !out.iter().any(|existing: &String| existing == &s) {
                out.push(s);
            }
        }
        self.pending_waypoint_labels = out;
    }

    /// C++ FollowWaypointPath success: keep the completed waypoint's labels.
    pub fn commit_completed_waypoint_labels(&mut self) {
        if !self.pending_waypoint_labels.is_empty() {
            self.completed_waypoint_labels = std::mem::take(&mut self.pending_waypoint_labels);
        }
    }

    pub fn clear_pending_waypoint_labels(&mut self) {
        self.pending_waypoint_labels.clear();
    }

    pub fn set_status_disabled_hacked(&mut self, v: bool) {
        self.status.disabled_hacked = v;
        crate::game_logic::host_status_log::record_disabled_hacked(self.id, v);
    }

    pub fn set_status_disabled_unmanned(&mut self, v: bool) {
        self.status.disabled_unmanned = v;
        crate::game_logic::host_status_log::record_disabled_unmanned(self.id, v);
    }

    pub fn set_status_disabled_paralyzed(&mut self, v: bool) {
        self.status.disabled_paralyzed = v;
        crate::game_logic::host_status_log::record_disabled_paralyzed(self.id, v);
    }

    pub fn set_status_disabled_subdued(&mut self, v: bool) {
        self.status.disabled_subdued = v;
        crate::game_logic::host_status_log::record_disabled_subdued(self.id, v);
    }

    /// C++ Object::setStatus / clearStatus residual via StatusBitsUpgrade.
    pub fn apply_status_bits_upgrade_masks(
        &mut self,
        set_names: &[&str],
        clear_names: &[&str],
    ) -> (u32, u32) {
        use crate::game_logic::host_status_bits_upgrade::{
            apply_status_bits_upgrade, object_status_mask_from_names, status_bits_has,
        };
        let before = self.object_status_bits;
        let uc_before =
            self.status.under_construction || status_bits_has(before, "UNDER_CONSTRUCTION");
        self.object_status_bits =
            apply_status_bits_upgrade(self.object_status_bits, set_names, clear_names);
        // Mirror a few high-traffic bits onto ObjectStatus bools.
        if status_bits_has(self.object_status_bits, "DESTROYED") {
            self.status.destroyed = true;
        }
        if status_bits_has(self.object_status_bits, "UNDER_CONSTRUCTION") {
            self.status.under_construction = true;
        } else if set_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("UNDER_CONSTRUCTION"))
            || clear_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("UNDER_CONSTRUCTION"))
        {
            // cleared path
            if !status_bits_has(self.object_status_bits, "UNDER_CONSTRUCTION") {
                self.status.under_construction = false;
            }
        }
        let uc_after = self.status.under_construction
            || status_bits_has(self.object_status_bits, "UNDER_CONSTRUCTION");
        if uc_before != uc_after {
            crate::game_logic::host_status_log::request_under_construction_mine_sweep(self.id);
        }
        if status_bits_has(self.object_status_bits, "REPULSOR") {
            self.status.repulsor = true;
        }
        if status_bits_has(self.object_status_bits, "SOLD") {
            // best-effort: sold residual if field exists
            let _ = self.status.sold;
            self.status.sold = true;
        }
        let set_m = object_status_mask_from_names(set_names);
        let clear_m = object_status_mask_from_names(clear_names);
        let set_count = set_m.count_ones();
        let clear_count = (before & clear_m).count_ones();
        (set_count, clear_count)
    }

    pub fn has_object_status_bit(&self, name: &str) -> bool {
        crate::game_logic::host_status_bits_upgrade::status_bits_has(self.object_status_bits, name)
    }

    /// C++ FlammableUpdate tryToIgnite / update status + model + body setAflame.
    pub fn apply_flammable_visuals(&mut self, aflame: bool, smoldering: bool, burned: bool) {
        if aflame {
            let _ = self.apply_status_bits_upgrade_masks(&["AFLAME"], &[]);
        } else if burned {
            let _ = self.apply_status_bits_upgrade_masks(&["BURNED"], &["AFLAME"]);
        } else {
            let _ = self.apply_status_bits_upgrade_masks(&[], &["AFLAME", "BURNED"]);
        }
        if smoldering {
            let _ = self.apply_status_bits_upgrade_masks(&["BURNED"], &[]);
        }
        if let Some(fs) = self.fire_spread.as_mut() {
            fs.body_aflame = aflame;
            if !aflame {
                fs.smoldering = burned;
            } else if smoldering {
                fs.smoldering = true;
            }
        }
        self.refresh_model_condition_bits();
    }

    /// C++ tryToIgnite: AFLAME status, body setAflame, MODELCONDITION_AFLAME.
    pub fn apply_flammable_ignite_visuals(&mut self) {
        self.apply_flammable_visuals(true, false, false);
    }

    /// C++ burned timer: BURNED + SMOLDERING while still AFLAME.
    pub fn apply_flammable_smoldering_visuals(&mut self) {
        self.apply_flammable_visuals(true, true, false);
    }

    /// C++ aflame_end: clear AFLAME / setAflame(FALSE); keep BURNED if already burned.
    pub fn apply_flammable_extinguish_visuals(&mut self, burned: bool) {
        self.apply_flammable_visuals(false, burned, burned);
    }

    /// C++ `Object::isScriptUnsellable`.
    pub fn is_script_unsellable(&self) -> bool {
        self.script_unsellable
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_UNSELLABLE)`.
    pub fn set_script_unsellable(&mut self, v: bool) {
        self.script_unsellable = v;
    }

    /// C++ `Object::hasSingleUseCommandBeenUsed`.
    pub fn has_single_use_command_been_used(&self) -> bool {
        self.single_use_command_used
    }

    /// C++ `Object::markSingleUseCommandUsed`.
    pub fn mark_single_use_command_used(&mut self) {
        self.single_use_command_used = true;
    }

    /// C++ `OBJECT_STATUS_SCRIPT_UNSTEALTHED`.
    pub fn is_script_unstealthed(&self) -> bool {
        self.script_unstealthed
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_UNSTEALTHED, !enabled)`.
    pub fn set_script_unstealthed(&mut self, v: bool) {
        self.script_unstealthed = v;
    }

    /// C++ `OBJECT_STATUS_SCRIPT_TARGETABLE` (map `objectTargetable`).
    pub fn is_script_targetable(&self) -> bool {
        self.script_targetable || leftover_object_script_targetable(self.id)
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_TARGETABLE)`.
    pub fn set_script_targetable(&mut self, v: bool) {
        self.script_targetable = v;
    }

    /// C++ `ActiveBody::isIndestructible`.
    pub fn is_indestructible(&self) -> bool {
        self.indestructible
    }

    /// C++ `ActiveBody::setIndestructible` flag write (tower mirror is GameLogic).
    pub fn set_indestructible(&mut self, v: bool) {
        self.indestructible = v;
    }

    /// C++ DISABLED_SCRIPT_DISABLED residual.
    pub fn set_status_disabled_script_disabled(&mut self, v: bool) {
        self.status.disabled_script_disabled = v;
    }

    /// C++ DISABLED_SCRIPT_UNDERPOWERED residual.
    pub fn set_status_disabled_script_underpowered(&mut self, v: bool) {
        self.status.disabled_script_underpowered = v;
    }

    /// C++ DISABLED_HELD residual (Battle Bus second life / contain freeze).
    pub fn set_status_disabled_held(&mut self, v: bool) {
        self.status.disabled_held = v;
    }

    /// C++ OBJECT_STATUS_MISSILE_KILLING_SELF residual.
    pub fn set_status_missile_killing_self(&mut self, v: bool) {
        self.status.missile_killing_self = v;
    }

    pub fn is_missile_killing_self(&self) -> bool {
        self.status.missile_killing_self
    }

    pub fn is_script_disabled(&self) -> bool {
        self.status.disabled_script_disabled
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_DISABLED, disabled)`.
    /// Leftover `Object::set_script_status` is RIGHT (partition dirty + setDisabled).
    pub fn set_script_disabled(&mut self, disabled: bool) {
        leftover_set_script_status(
            self.id,
            gamelogic::object::ObjectScriptStatusBit::ScriptDisabled,
            disabled,
        );
        self.apply_script_status_disabled_side_effects(disabled);
    }

    pub fn is_script_underpowered(&self) -> bool {
        self.status.disabled_script_underpowered
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_UNPOWERED, underpowered)`.
    /// Leftover `Object::set_script_status` is RIGHT (partition dirty + setDisabled).
    pub fn set_script_underpowered(&mut self, underpowered: bool) {
        leftover_set_script_status(
            self.id,
            gamelogic::object::ObjectScriptStatusBit::ScriptUnderpowered,
            underpowered,
        );
        self.apply_script_status_underpowered_side_effects(underpowered);
    }

    fn apply_script_status_disabled_side_effects(&mut self, disabled: bool) {
        if self.status.disabled_script_disabled == disabled {
            return;
        }
        let was_disabled = self.is_disabled();
        self.set_status_disabled_script_disabled(disabled);
        self.handle_partition_cell_maintenance();
        let now_disabled = self.is_disabled();
        if !was_disabled && now_disabled {
            self.on_disabled_edge(true);
        } else if was_disabled && !now_disabled {
            self.on_disabled_edge(false);
        }
    }

    fn apply_script_status_underpowered_side_effects(&mut self, underpowered: bool) {
        if self.status.disabled_script_underpowered == underpowered {
            return;
        }
        let was_disabled = self.is_disabled();
        self.set_status_disabled_script_underpowered(underpowered);
        self.handle_partition_cell_maintenance();
        let now_disabled = self.is_disabled();
        if !was_disabled && now_disabled {
            self.on_disabled_edge(true);
        } else if was_disabled && !now_disabled {
            self.on_disabled_edge(false);
        }
    }

    pub fn is_held_disabled(&self) -> bool {
        self.status.disabled_held
    }

    /// C++ `ScriptActions::changeObjectPanelFlagForSingleObject` live residual.
    pub fn apply_object_panel_flag(&mut self, flag_to_change: &str, new_val: bool) {
        let normalized = flag_to_change
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && *c != '_')
            .collect::<String>()
            .to_ascii_lowercase();
        match normalized.as_str() {
            "enabled" => self.set_script_disabled(!new_val),
            "powered" => self.set_script_underpowered(!new_val),
            "unsellable" => self.set_script_unsellable(new_val),
            "indestructible" => self.set_indestructible(new_val),
            "playertargetable" => self.set_script_targetable(new_val),
            _ => {}
        }
    }

    /// C++ `Object::maskObject` — leftover `mask_object` is RIGHT (MASKED + deselect).
    pub fn set_status_masked(&mut self, v: bool) {
        leftover_mask_object(self.id, v);
        self.status.masked = v;
        crate::game_logic::host_status_log::record_masked(self.id, v);
        if v {
            self.deselect();
            request_mask_deselect(self.id);
        }
    }

    pub fn set_status_disguised(&mut self, v: bool) {
        self.status.disguised = v;
        crate::game_logic::host_status_log::record_disguised(self.id, v);
    }
    pub fn set_status_no_collisions(&mut self, v: bool) {
        self.status.no_collisions = v;
        crate::game_logic::host_status_log::record_no_collisions(self.id, v);
    }

    pub fn set_status_private_captured(&mut self, v: bool) {
        self.status.private_captured = v;
        crate::game_logic::host_status_log::record_private_captured(self.id, v);
    }

    pub fn set_status_disguise_transitioning_to(&mut self, v: bool) {
        self.status.disguise_transitioning_to = v;
        crate::game_logic::host_status_log::record_disguise_transitioning_to(self.id, v);
    }

    pub fn set_status_disguise_halfpoint_reached(&mut self, v: bool) {
        self.status.disguise_halfpoint_reached = v;
        crate::game_logic::host_status_log::record_disguise_halfpoint_reached(self.id, v);
    }

    pub fn set_status_faerie_fire(&mut self, v: bool) {
        self.status.faerie_fire = v;
        crate::game_logic::host_status_log::record_faerie_fire(self.id, v);
    }

    pub fn set_status_booby_trapped(&mut self, v: bool) {
        self.status.booby_trapped = v;
        crate::game_logic::host_status_log::record_booby_trapped(self.id, v);
    }

    pub fn set_status_eject_invulnerable(&mut self, v: bool) {
        self.status.eject_invulnerable = v;
        crate::game_logic::host_status_log::record_eject_invulnerable(self.id, v);
    }

    pub fn set_status_pilot_did_move_to_base(&mut self, v: bool) {
        self.status.pilot_did_move_to_base = v;
        crate::game_logic::host_status_log::record_pilot_did_move_to_base(self.id, v);
    }

    pub fn set_status_parachuting(&mut self, v: bool) {
        self.status.parachuting = v;
        crate::game_logic::host_status_log::record_parachuting(self.id, v);
    }

    pub fn set_status_parachute_open(&mut self, v: bool) {
        self.status.parachute_open = v;
        crate::game_logic::host_status_log::record_parachute_open(self.id, v);
    }

    pub fn set_status_parachute_landing_override_set(&mut self, v: bool) {
        self.status.parachute_landing_override_set = v;
        crate::game_logic::host_status_log::record_parachute_landing_override_set(self.id, v);
    }
    pub fn set_status_using_ability(&mut self, v: bool) {
        self.status.using_ability = v;
        crate::game_logic::host_status_log::record_using_ability(self.id, v);
    }
    pub fn set_status_deployed(&mut self, v: bool) {
        self.status.deployed = v;
        crate::game_logic::host_status_log::record_deployed(self.id, v);
    }
    pub fn set_status_under_construction(&mut self, v: bool) {
        if self.status.under_construction != v {
            crate::game_logic::host_status_log::request_under_construction_mine_sweep(self.id);
        }
        self.status.under_construction = v;
        crate::game_logic::host_status_log::record_under_construction(self.id, v);
    }
    pub fn set_status_sold(&mut self, v: bool) {
        self.status.sold = v;
        crate::game_logic::host_status_log::record_sold(self.id, v);
    }
    pub fn set_status_reconstructing(&mut self, v: bool) {
        self.status.reconstructing = v;
        crate::game_logic::host_status_log::record_reconstructing(self.id, v);
    }
    pub fn set_status_unselectable(&mut self, v: bool) {
        self.status.unselectable = v;
        crate::game_logic::host_status_log::record_unselectable(self.id, v);
    }
    pub fn set_status_ignoring_stealth(&mut self, v: bool) {
        self.status.ignoring_stealth = v;
        crate::game_logic::host_status_log::record_ignoring_stealth(self.id, v);
    }
    pub fn set_status_repulsor(&mut self, v: bool) {
        self.status.repulsor = v;
        crate::game_logic::host_status_log::record_repulsor(self.id, v);
        crate::game_logic::host_repulsor_log::record(self.id, v, self.repulsor_until_frame);
    }

    /// Arm temporary repulsor helper countdown (C++ ObjectRepulsorHelper residual).
    pub fn arm_repulsor_countdown(&mut self, remaining_frames: u32) {
        self.repulsor_until_frame = remaining_frames;
        self.set_status_repulsor(true);
    }
    pub fn set_status_disabled_underpowered(&mut self, v: bool) {
        self.status.disabled_underpowered = v;
        crate::game_logic::host_status_log::record_disabled_underpowered(self.id, v);
    }
    pub fn set_status_disabled_freefall(&mut self, v: bool) {
        self.status.disabled_freefall = v;
        crate::game_logic::host_status_log::record_disabled_freefall(self.id, v);
    }
    pub fn set_status_is_carbomb(&mut self, v: bool) {
        self.status.is_carbomb = v;
        crate::game_logic::host_status_log::record_is_carbomb(self.id, v);
    }
    pub fn set_status_hijacked(&mut self, v: bool) {
        self.status.hijacked = v;
        crate::game_logic::host_status_log::record_hijacked(self.id, v);
    }
    pub fn set_status_force_attack(&mut self, v: bool) {
        self.force_attack = v;
        crate::game_logic::host_status_log::record_force_attack(self.id, v);
    }
    pub fn record_host_guard(&self) {
        let position = self.guard_position.map(|p| [p.x, p.y, p.z]);
        let target_host = self.guard_target.map(|id| id.0).unwrap_or(0);
        crate::game_logic::host_guard_log::record(
            self.id,
            position,
            target_host,
            self.guard_radius,
        );
    }

    pub fn record_host_continuous_fire(&self) {
        let consecutive = self.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
        crate::game_logic::host_continuous_fire_log::record(
            self.id,
            self.continuous_fire_level,
            consecutive,
            self.continuous_fire_coast_until_frame,
        );
    }

    pub fn record_host_detector(&self) {
        crate::game_logic::host_detector_log::record(
            self.id,
            self.is_detector,
            self.detection_range,
            self.detection_rate_frames,
        );
    }

    pub fn set_detector_state(
        &mut self,
        is_detector: bool,
        detection_range: f32,
        detection_rate_frames: u32,
    ) {
        let detection_range = detection_range.max(0.0);
        if self.is_detector != is_detector
            || (self.detection_range - detection_range).abs() > 1e-5
            || self.detection_rate_frames != detection_rate_frames
        {
            self.is_detector = is_detector;
            self.detection_range = detection_range;
            self.detection_rate_frames = detection_rate_frames;
            self.record_host_detector();
        }
        if self.is_detector {
            self.apply_leftover_extra_detect_kindof();
        }
    }

    /// C++ `StealthDetectorUpdateModuleData` ExtraRequired/ForbiddenKindOf
    /// from leftover ThingFactory (StealthDetectorUpdate.cpp:53-54, :168).
    pub fn apply_leftover_extra_detect_kindof(&mut self) {
        let (required, forbidden) =
            crate::game_logic::host_radar_stealth_vision_residual::extra_detect_kindof_for_detector(
                &self.template_name,
                self.extra_detect_kindof,
                self.extra_detect_kindof_not,
            );
        self.extra_detect_kindof = required;
        self.extra_detect_kindof_not = forbidden;
    }

    /// C++ `StealthDetectorUpdate` ctor: random first wake so detectors do not
    /// all scan / IR-ping on the same frame.
    ///
    /// `setSDEnabled(true)` stays `next_detection_scan_frame = 0` (immediate).
    pub fn apply_stealth_detector_ctor_stagger(&mut self, frame: u32) {
        if !self.is_detector {
            return;
        }
        if self.detection_rate_frames == 0 {
            self.detection_rate_frames =
                crate::game_logic::host_strategy_center::stealth_detector_rate_frames_for_template(
                    &self.template_name,
                );
            self.record_host_detector();
        }
        self.apply_leftover_extra_detect_kindof();
        self.next_detection_scan_frame =
            crate::game_logic::host_strategy_center::stealth_detector_ctor_next_scan_frame(
                self.detection_rate_frames,
                frame,
            );
    }

    pub fn record_host_target_location(&self) {
        let loc = self.target_location.map(|p| [p.x, p.y, p.z]);
        crate::game_logic::host_target_location_log::record(self.id, loc);
    }

    pub fn record_host_hijacker(&self) {
        crate::game_logic::host_hijacker_log::record(
            self.id,
            self.hijack_vehicle_id.map(|id| id.0).unwrap_or(0),
            self.hijacker_in_vehicle,
            self.hijacker_update_active,
            self.hijacker_was_airborne,
            self.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]),
            self.hive_slave_respawn_frame,
            self.next_detection_scan_frame,
        );
    }

    pub fn record_host_ai_request(&self) {
        let pending_team = self
            .disguise_pending_team
            .map(|t| match t {
                crate::game_logic::Team::USA => 0u8,
                crate::game_logic::Team::China => 1u8,
                crate::game_logic::Team::GLA => 2u8,
                crate::game_logic::Team::Neutral => 3u8,
            })
            .unwrap_or(255u8);
        crate::game_logic::host_ai_request_log::record(
            self.id,
            self.requested_victim_id.map(|id| id.0).unwrap_or(0),
            self.requested_destination.map(|p| [p.x, p.y, p.z]),
            self.prev_victim_pos.map(|p| [p.x, p.y, p.z]),
            self.crate_created.map(|id| id.0).unwrap_or(0),
            self.guard_retaliate_victim.map(|id| id.0).unwrap_or(0),
            self.guard_retaliate_anchor.map(|p| [p.x, p.y, p.z]),
            self.path_timestamp,
            self.disguise_pending_template.clone().unwrap_or_default(),
            pending_team,
            self.weapon_crate_upgrade,
            self.armor_crate_upgrade,
            self.selection_flash_remaining,
        );
    }

    pub fn record_host_locomotor(&self) {
        crate::game_logic::host_locomotor_log::record(
            self.id,
            self.is_approach_path,
            self.on_invalid_movement_terrain,
            self.was_airborne_last_frame,
            self.can_move_backward,
            self.moving_backwards,
            self.no_slow_down_as_approaching_dest,
            self.turn_pivot_offset,
            self.wander_width_factor,
            self.loco_apply_2d_friction_airborne,
            self.allow_motive_force_while_airborne,
            self.loco_extra_2d_friction,
            self.loco_preferred_height,
            self.loco_preferred_height_damping,
            self.loco_appearance.to_ordinal(),
            self.loco_behavior_z.to_ordinal(),
            self.min_turn_speed,
            self.physics_turning.to_ordinal(),
        );
    }

    pub fn record_host_fire_intent(&self) {
        crate::game_logic::host_fire_intent_log::record(
            self.id,
            self.last_fire_victim_host,
            self.last_fire_slot,
            self.last_fire_damage,
            self.last_fire_range,
            self.last_fire_sim_time,
            self.last_fire_frame,
            self.fire_intent_count,
        );
    }

    pub fn record_host_combat_attack(&self) {
        crate::game_logic::host_combat_attack_log::record(
            self.id,
            self.pre_attack_target.map(|id| id.0).unwrap_or(0),
            self.pre_attack_ready_at,
            self.consecutive_shots_at_target,
            self.max_shots_to_fire,
            self.attack_substate.to_ordinal(),
            self.approach_timestamp,
            self.continuous_fire_victim,
            self.maintain_pos_valid,
            self.maintain_pos.map(|p| [p.x, p.y, p.z]),
            self.temporary_move_frames,
            self.group_speed_factor,
        );
    }

    pub fn record_host_stealth_delay(&self) {
        crate::game_logic::host_stealth_delay_log::record(
            self.id,
            self.stealth_allowed_frame,
            self.stealth_delay_pending,
            self.stealth_delay_frames,
            self.stealth_breaks_on_damage,
            self.detection_expires_frame,
            self.camo_opacity_pulse_phase,
            self.camo_heat_vision_opacity,
            self.camo_net_sub_object_shown,
            self.camo_net_sub_object_observer_visible,
        );
    }

    pub fn record_host_turret(&self) {
        crate::game_logic::host_turret_log::record(
            self.id,
            self.turret_angle_deg,
            self.turret_pitch_deg,
            self.turret_holding,
            self.turret_idle_scanning,
            self.turret_turn_rate_rad,
            self.turret_recenter_frames,
            self.turret_hold_until_frame,
            self.turret_idle_recentering,
            self.turret_enabled,
            self.turret_rotating,
            self.turret_natural_angle_deg,
            self.turret_natural_pitch_deg,
            self.turret_target_id.map(|id| id.0).unwrap_or(0),
            self.turret_force_attacking,
            self.turret_mood_target,
            self.turret_idle_scan_next_frame,
            self.turret_idle_scan_desired_angle_deg,
            self.turret_idle_scan_index,
            self.turret_substate.ordinal(),
        );
    }

    pub fn record_host_entity_power(&self) {
        crate::game_logic::host_entity_power_log::record(
            self.id,
            self.power_provided,
            self.power_consumed,
        );
    }

    pub fn set_entity_power(&mut self, provided: i32, consumed: i32) {
        let provided = provided.max(0);
        let consumed = consumed.max(0);
        if self.power_provided != provided || self.power_consumed != consumed {
            self.power_provided = provided;
            self.power_consumed = consumed;
            self.record_host_entity_power();
        }
    }

    pub fn record_host_weapon_slot(&self) {
        crate::game_logic::host_weapon_slot_log::record(self.id, self.active_weapon_slot);
    }

    /// C++ Object::setWeaponLock residual.
    /// Returns false if the requested slot has no weapon.
    pub fn set_weapon_lock(&mut self, slot: u8, lock_type: WeaponLockType) -> bool {
        if lock_type == WeaponLockType::NotLocked {
            self.release_weapon_lock(WeaponLockType::LockedPermanently);
            return true;
        }
        if self.weapon_slot(slot).is_none() {
            return false;
        }
        // Permanent lock cannot be overridden by temporary (C++ WeaponSet residual).
        if self.weapon_lock_type == WeaponLockType::LockedPermanently
            && lock_type == WeaponLockType::LockedTemporarily
        {
            return false;
        }
        self.weapon_lock_type = lock_type;
        self.weapon_lock_slot = slot;
        self.set_active_weapon_slot(slot);
        true
    }

    /// C++ Object::releaseWeaponLock residual.
    pub fn release_weapon_lock(&mut self, lock_type: WeaponLockType) {
        match lock_type {
            WeaponLockType::NotLocked => {}
            WeaponLockType::LockedTemporarily => {
                if self.weapon_lock_type == WeaponLockType::LockedTemporarily {
                    self.weapon_lock_type = WeaponLockType::NotLocked;
                    // `active_weapon_slot == 2` is the host's explicit-manual
                    // marker for a manual-only tertiary weapon.  Once a C++
                    // temporary lock has completed or been cancelled, retain
                    // neither that marker nor the special attack selection:
                    // the next ordinary chooser may consider only PRIMARY /
                    // SECONDARY again.  Slot zero is deliberately used even
                    // when absent; the chooser then falls through to an
                    // existing SECONDARY rather than inventing TERTIARY.
                    self.set_active_weapon_slot(0);
                }
            }
            WeaponLockType::LockedPermanently => {
                // Permanent release clears any lock.
                self.weapon_lock_type = WeaponLockType::NotLocked;
            }
        }
    }

    pub fn is_weapon_locked(&self) -> bool {
        self.weapon_lock_type != WeaponLockType::NotLocked
    }

    /// C++ Drawable::setEmoticon residual (duration in logic frames @ 30Hz).
    pub fn set_surrendered(&mut self, surrendered: bool) {
        self.is_surrendered = surrendered;
        if surrendered {
            self.stop_moving();
            self.set_target(None);
            self.set_force_attack(false);
            self.set_ai_state(AIState::Idle);
        }
    }

    pub fn set_emoticon(&mut self, name: &str, duration_frames: i32) {
        // C++ Drawable::setEmoticon: duration < 0 is FOREVER; 0 clears.
        if name.is_empty() || duration_frames == 0 {
            self.emoticon_name.clear();
            self.emoticon_frames_left = 0;
            return;
        }
        self.emoticon_name = name.to_string();
        self.emoticon_frames_left = if duration_frames < 0 {
            i32::MAX
        } else {
            duration_frames
        };
    }

    /// C++ `Drawable::setFlashCount` / `setFlashColor`.
    pub fn set_script_flash(&mut self, seconds: i32, color: u32) {
        if seconds <= 0 {
            return;
        }
        let frames = 30i32.saturating_mul(seconds);
        let count = frames / 15; // C++ DRAWABLE_FRAMES_PER_FLASH
        if count <= 0 {
            return;
        }
        self.flash_count = count;
        self.flash_color = color;
    }

    /// C++ `Object::setCustomIndicatorColor` (`0` removes custom color).
    pub fn set_custom_indicator_color_raw(&mut self, color_raw: u32) {
        self.custom_indicator_color = if color_raw == 0 {
            None
        } else {
            Some(color_raw)
        };
    }

    pub fn set_active_weapon_slot(&mut self, slot: u8) {
        if self.active_weapon_slot != slot {
            self.active_weapon_slot = slot;
            self.record_host_weapon_slot();
        }
    }

    pub fn record_host_special_power(&self) {
        crate::game_logic::host_special_power_log::record(
            self.id,
            self.special_power_ready,
            self.special_power_cooldown_remaining,
            self.special_power_cooldown,
            self.is_disabled(),
        );
    }

    pub fn set_special_power_ready(&mut self, ready: bool) {
        self.special_power_ready = ready;
        if ready {
            // Force-ready is the port's test/script express-ready cheat.  The
            // authoritative per-module gate is the cooldown map
            // (Object::is_special_power_ready reads special_power_cooldowns),
            // so expressing readiness must clear it — otherwise a caster
            // created after C++ SpecialPowerModule ctor arming
            // (SpecialPowerModule.cpp:86-94) could never cast at frame 0.
            self.special_power_cooldowns.clear();
        }
        self.record_host_special_power();
    }

    pub fn set_stored_supplies(&mut self, supplies: u32) {
        self.stored_resources.supplies = supplies;
        crate::game_logic::host_stored_supplies_log::record(self.id, supplies);
        if self.thing.template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse {
            let (max_boxes, current_boxes) =
                crate::game_logic::host_supply_gather::drawable_supply_status_from_cash(
                    self.thing.template.dock_starting_boxes,
                    supplies,
                );
            self.drawable_supply_max_boxes = max_boxes;
            self.drawable_supply_boxes = current_boxes;
        } else if let Some(metadata) = self.thing.template.supply_truck_metadata {
            // C++ SupplyTruckAIUpdate::gainOneBox / loseOneBox:
            // drawable->updateDrawableSupplyStatus(maxBoxes, m_numberBoxes).
            let (max_boxes, current_boxes) =
                crate::game_logic::host_supply_gather::collector_drawable_supply_status(
                    metadata.max_boxes,
                    supplies,
                );
            self.drawable_supply_max_boxes = max_boxes;
            self.drawable_supply_boxes = current_boxes;
        }
    }

    /// Set the AI state for autonomous behavior
    /// Wave 630: apply combat-status residual after GW AI-state writeback.
    ///
    /// Does not re-log host_ai_state (state already last-written). Syncs
    /// moving/attacking status bits from the authoritative ordinal.
    pub(crate) fn apply_ai_state_combat_status_residual(&mut self, ordinal: u8) {
        let moving = matches!(ordinal, 1 | 3); // Moving | AttackMoving
        let attacking = matches!(ordinal, 2 | 3 | 4 | 20); // Attacking | AttackMoving | AttackingGround | GuardRetaliating
        self.set_status_moving(moving);
        self.set_status_attacking(attacking);
        if !moving {
            // Clear residual velocity when leaving march states.
            if matches!(ordinal, 0 | 12 | 13) {
                // Idle | Docked | Garrisoned
                self.movement.velocity = glam::Vec3::ZERO;
            }
        }
        self.record_host_movement();
    }

    pub fn set_ai_state(&mut self, state: AIState) {
        // C++ `StateMachine::internalSetState` fires `onExit(EXIT_RESET)` then
        // `AIDockMachine::halt` → `cancelDock`. Leaving a live dock session
        // must free the approach slot so a re-tasked truck cannot ghost-jam.
        if crate::game_logic::host_supply_gather::is_live_dock_ai_state(&self.ai_state)
            && self.ai_state != state
        {
            crate::game_logic::host_supply_gather::cancel_live_dock_for_docker(self.id);
        }
        let was_entering = matches!(self.ai_state, AIState::Entering);
        let ordinal = match state {
            AIState::Idle => 0u8,
            AIState::Moving => 1,
            AIState::Attacking => 2,
            AIState::AttackMoving => 3,
            AIState::AttackingGround => 4,
            AIState::Gathering => 5,
            AIState::ReturningResources => 6,
            AIState::Constructing => 7,
            AIState::Repairing => 8,
            AIState::GuardingArea => 9,
            AIState::GuardingObject => 10,
            AIState::Patrolling => 11,
            AIState::Docked => 12,
            AIState::Garrisoned => 13,
            AIState::SpecialAbility => 14,
            AIState::SeekingRepair => 15,
            AIState::SeekingHealing => 16,
            AIState::Entering => 17,
            AIState::Docking => 18,
            AIState::Capturing => 19,
            AIState::GuardRetaliating => 20,
            AIState::FacingObject => 21,
            AIState::FacingPosition => 22,
        };
        if !matches!(
            state,
            AIState::FacingObject | AIState::FacingPosition | AIState::SpecialAbility
        ) && self.face_active
        {
            self.face_active = false;
            if self.locomotor_goal_type == LocoGoalType::Angle {
                self.set_locomotor_goal_none();
            }
        }
        self.ai_state = state;
        if matches!(self.ai_state, AIState::Constructing | AIState::Repairing) {
            // Assignment: ULTRA_ACCURATE on dozer/worker precision approach.
            self.set_ultra_accurate(true);
        }
        // C++ AIEnterState::onEnter / onExit setAllowInvalidPosition
        // (AIStates.cpp:6227, :6247) so infantry are not 3x3-shoved off
        // building cells while walking in.
        if matches!(self.ai_state, AIState::Entering) {
            self.set_allow_invalid_position(true);
        } else if was_entering {
            self.set_allow_invalid_position(false);
        }
        crate::game_logic::host_ai_state_log::record(self.id, ordinal);
        self.record_host_ai_mood();
    }
}

/// Drain leftover `OBJECT_STATUS_SCRIPT_TARGETABLE` (map `objectTargetable` /
/// leftover_sa `Player Targetable` / OCL DeliverPayload transport).
pub fn leftover_object_script_targetable(id: ObjectId) -> bool {
    gamelogic::object::registry::OBJECT_REGISTRY
        .with_object(id.0, |obj| {
            obj.test_script_status_bit(gamelogic::object::ObjectScriptStatusBit::ScriptTargetable)
        })
        .unwrap_or(false)
}

fn leftover_mask_object(id: ObjectId, mask: bool) {
    let _ = gamelogic::object::registry::OBJECT_REGISTRY.with_object_mut(id.0, |obj| {
        obj.mask_object(mask);
    });
}

fn leftover_set_script_status(
    id: ObjectId,
    bit: gamelogic::object::ObjectScriptStatusBit,
    set: bool,
) {
    let _ = gamelogic::object::registry::OBJECT_REGISTRY.with_object_mut(id.0, |obj| {
        obj.set_script_status(bit, set);
    });
}

/// Drain leftover `Object::is_hero` (contained KINDOF_HERO, else self).
pub fn leftover_object_is_hero(id: ObjectId) -> bool {
    gamelogic::object::registry::OBJECT_REGISTRY
        .with_object(id.0, |obj| obj.is_hero())
        .unwrap_or(false)
}

/// Drain leftover `is_kind_of(KINDOF_HERO)` for a contained occupant id.
pub fn leftover_object_is_kind_of_hero(id: ObjectId) -> bool {
    gamelogic::object::registry::OBJECT_REGISTRY
        .with_object(id.0, |obj| obj.is_kind_of(gamelogic::common::KindOf::Hero))
        .unwrap_or(false)
}

thread_local! {
    static MASK_DESELECTS: std::cell::RefCell<Vec<ObjectId>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Queue a live player-list deselect for C++ `maskObject(TRUE)`.
pub fn request_mask_deselect(id: ObjectId) {
    MASK_DESELECTS.with(|q| q.borrow_mut().push(id));
}

/// Drain leftover `maskObject` deselects onto live player selection.
pub fn drain_mask_deselects() -> Vec<ObjectId> {
    MASK_DESELECTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}
