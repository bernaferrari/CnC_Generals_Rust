use super::*;

impl Object {
    /// Read the durable accepted-discharge marker without exposing the older
    /// AI fire-intent presentation fields as a visual authority.
    #[inline]
    pub fn weapon_discharge_marker(&self) -> WeaponDischargeMarker {
        WeaponDischargeMarker {
            sequence: self.last_weapon_discharge_sequence,
            weapon_slot: self.last_weapon_discharge_slot,
            fired_barrel: self.last_weapon_discharge_barrel,
            logic_frame: self.last_weapon_discharge_frame,
        }
    }

    /// Restore a v4 logical marker. Unknown slots and sequence zero fail
    /// closed to the unseen state; a renderer must not replay a malformed
    /// source record as PRIMARY/barrel zero.
    pub fn restore_weapon_discharge_marker(
        &mut self,
        sequence: u64,
        weapon_slot: u8,
        fired_barrel: u8,
        logic_frame: u32,
    ) -> bool {
        if sequence == 0 || weapon_slot >= 3 {
            self.last_weapon_discharge_sequence = 0;
            self.last_weapon_discharge_slot = 0;
            self.last_weapon_discharge_barrel = 0;
            self.last_weapon_discharge_frame = 0;
            return false;
        }
        self.last_weapon_discharge_sequence = sequence;
        self.last_weapon_discharge_slot = weapon_slot;
        self.last_weapon_discharge_barrel = fired_barrel;
        self.last_weapon_discharge_frame = logic_frame;
        true
    }

    /// Stamp an event allocated by the owning `GameLogic` world.
    pub(crate) fn stamp_weapon_discharge_marker(&mut self, marker: WeaponDischargeMarker) {
        let _ = self.restore_weapon_discharge_marker(
            marker.sequence,
            marker.weapon_slot,
            marker.fired_barrel,
            marker.logic_frame,
        );
    }

    pub fn record_host_ground_height(&self) {
        crate::game_logic::host_ground_height_log::record(
            self.id,
            self.ground_height,
            self.ground_height_from_terrain,
        );
    }

    pub fn set_ground_height_residual(&mut self, height: f32, from_terrain: bool) {
        let changed = (self.ground_height - height).abs() > f32::EPSILON
            || self.ground_height_from_terrain != from_terrain;
        if !changed {
            return;
        }
        self.ground_height = height;
        self.ground_height_from_terrain = from_terrain;
        self.record_host_ground_height();
    }

    /// Presentation mesh identity residual (model_key + mesh_scale) → GameWorld SetModelMesh.
    pub fn set_model_mesh_residual(&mut self, model_key: impl Into<String>, mesh_scale: f32) {
        let key = model_key.into();
        let scale = if mesh_scale.is_finite() && mesh_scale > 0.0 {
            mesh_scale
        } else {
            1.0
        };
        crate::game_logic::host_model_mesh_log::record(self.id, key, scale);
    }

    /// Resolve and log mesh residual from the active (possibly disguised) template.
    pub fn record_model_mesh_from_template(&mut self) {
        let tpl = self.get_template();
        let key = crate::assets::mesh_asset_resolve::model_key_from_template(tpl);
        let scale = crate::assets::mesh_asset_resolve::mesh_scale_from_template(tpl);
        self.set_model_mesh_residual(key, scale);
    }

    /// FOW visibility residual → GameWorld SetFow (presentation last-writer channel).
    pub fn set_fow_residual(
        &mut self,
        visibility_alpha: f32,
        is_explored: f32,
        visibility_falloff: f32,
    ) {
        crate::game_logic::host_fow_log::record(
            self.id,
            visibility_alpha,
            is_explored,
            visibility_falloff,
        );
    }

    /// Presentation kind_of ORDER bits residual (same ORDER as GameWorldShadow::host_kind_of_bits).
    pub fn presentation_kind_of_bits(&self) -> u32 {
        use crate::game_logic::KindOf;
        const ORDER: &[KindOf] = &[
            KindOf::Structure,
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Aircraft,
            KindOf::Projectile,
            KindOf::Resource,
            KindOf::Selectable,
            KindOf::Attackable,
            KindOf::CommandCenter,
            KindOf::Worker,
            KindOf::Hero,
            KindOf::SupplyCenter,
            KindOf::PowerPlant,
            KindOf::FSBarracks,
            KindOf::FSWarFactory,
            KindOf::FSAirfield,
            KindOf::FSInternetCenter,
            KindOf::FSPower,
            KindOf::FSBaseDefense,
            KindOf::FSSupplyDropzone,
            KindOf::FSSupplyCenter,
            KindOf::FSSuperweapon,
            KindOf::FSStrategyCenter,
            KindOf::FSFake,
            KindOf::FSTechnology,
            KindOf::FSBlackMarket,
            KindOf::FSAdvancedTech,
            KindOf::Harvestable,
            KindOf::Powered,
            KindOf::IgnoredInGui,
            // Appended to preserve every pre-existing presentation bit index.
            KindOf::Dozer,
            KindOf::Harvester,
        ];
        let set = &self.get_template().kind_of;
        let mut bits = 0u32;
        for (i, k) in ORDER.iter().enumerate() {
            if set.contains(k) {
                bits |= 1u32 << i;
            }
        }
        bits
    }

    /// kind_of bits residual → GameWorld SetKindOfBits.
    pub fn set_kind_of_bits_residual(&mut self, kind_of_bits: u32) {
        crate::game_logic::host_kind_of_log::record(self.id, kind_of_bits);
    }

    /// Resolve and log kind_of bits from the active template.
    pub fn record_kind_of_bits_from_template(&mut self) {
        let bits = self.presentation_kind_of_bits();
        self.set_kind_of_bits_residual(bits);
    }

    pub fn record_host_identity(&self) {
        crate::game_logic::host_identity_log::record(self.id, self.name.clone(), self.team_color);
    }

    pub fn record_host_building_type(&self) {
        use crate::game_logic::BuildingType as B;
        let (is_building, ordinal) = match self.building_data.as_ref() {
            Some(bd) => {
                let ord = match bd.building_type {
                    B::CommandCenter => 0u8,
                    B::Barracks => 1,
                    B::WarFactory => 2,
                    B::Airfield => 3,
                    B::RepairPad => 4,
                    B::HealPad => 5,
                    B::SupplyCenter => 6,
                    B::PowerPlant => 7,
                    B::DefenseTurret => 8,
                    B::SupplyDropZone => 9,
                    B::Palace => 10,
                    B::Propaganda => 11,
                    B::Bunker => 12,
                };
                (true, ord)
            }
            None => (false, 255u8),
        };
        crate::game_logic::host_building_type_log::record(self.id, is_building, ordinal);
    }

    /// C++ CrushDie model condition FRONTCRUSHED/BACKCRUSHED residual.
    pub fn apply_crush_die_model_conditions(&mut self) {
        use crate::game_logic::host_neutron_missile_slow_death::{
            MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
        };
        let before = self.model_condition_bits;
        // Clear then set like C++ clearAndSetModelConditionFlags.
        self.model_condition_bits &= !(1u128 << MC_BIT_FRONTCRUSHED);
        self.model_condition_bits &= !(1u128 << MC_BIT_BACKCRUSHED);
        if self.front_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_FRONTCRUSHED;
        }
        if self.back_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_BACKCRUSHED;
        }
        // Wave 487: crush model bits must reach GW before model-condition writeback.
        if self.model_condition_bits != before {
            self.record_host_model_condition();
        }
    }

    pub fn record_host_crush_vision(&self) {
        crate::game_logic::host_crush_vision_log::record(
            self.id,
            self.crusher_level,
            self.crushable_level,
            self.vision_range,
            self.shroud_clearing_range,
            self.front_crushed,
            self.back_crushed,
        );
    }

    pub fn record_host_demo_mine_cheer(&self) {
        crate::game_logic::host_demo_mine_cheer_log::record(
            self.id,
            self.demo_suicided_detonating,
            self.mine_data.is_some(),
            self.cheer_timer,
        );
    }

    pub fn set_formation(&mut self, formation_id: u32, formation_offset: glam::Vec2) {
        self.formation_id = formation_id;
        self.formation_offset = formation_offset;
        // Wave 204: last-write SetFormation.
        crate::game_logic::host_formation_log::record(
            self.id,
            self.formation_id,
            [self.formation_offset.x, self.formation_offset.y],
        );
    }

    pub fn begin_cheer(&mut self, duration_secs: f32, cheer_bit: Option<usize>) {
        self.set_ai_state(AIState::SpecialAbility);
        self.cheer_timer = duration_secs.max(0.0);
        if let Some(bit) = cheer_bit {
            if bit < 128 {
                self.model_condition_bits |= 1u128 << bit;
                self.record_host_model_condition();
            }
        }
        // Wave 202: last-write SetDemoMineCheer (cheer_timer residual).
        self.record_host_demo_mine_cheer();
    }

    pub fn record_host_selection_radius(&self) {
        crate::game_logic::host_selection_radius_log::record(self.id, self.selection_radius);
    }

    pub fn set_selection_radius(&mut self, selection_radius: f32) {
        if (self.selection_radius - selection_radius).abs() > f32::EPSILON {
            self.selection_radius = selection_radius;
            self.record_host_selection_radius();
        }
    }

    pub fn record_host_model_condition(&self) {
        crate::game_logic::host_model_condition_log::record(self.id, self.model_condition_bits);
    }

    pub fn record_host_radar_extend(&self) {
        crate::game_logic::host_radar_extend_log::record(
            self.id,
            self.radar_extend_done_frame,
            self.radar_extend_complete,
            self.radar_active,
        );
    }

    pub fn record_host_shock_stun(&self) {
        crate::game_logic::host_shock_stun_log::record(
            self.id,
            self.shock_stun_frames,
            self.shock_yaw_rate,
            self.shock_pitch_rate,
            self.shock_roll_rate,
            self.shock_up_z,
            self.shock_allow_bounce,
            self.shock_grounded_once,
            self.shock_was_airborne,
            self.cell_is_cliff,
            self.cell_is_underwater,
        );
    }

    pub fn record_host_production_door(&self) {
        crate::game_logic::host_production_door_log::record(
            self.id,
            self.production_door_phase,
            self.production_door_phase_end_frame,
            self.production_door_hold_open,
        );
    }

    pub fn record_host_ai_mood(&self) {
        crate::game_logic::host_ai_mood_log::record(
            self.id,
            self.idle_since_frame,
            self.mood_attack_check_rate,
            self.auto_acquire_when_idle,
            self.attack_priority_set.clone().unwrap_or_default(),
        );
    }

    pub fn record_host_sole_healing(&self) {
        crate::game_logic::host_sole_healing_log::record(
            self.id,
            self.sole_healing_benefactor.map(|id| id.0),
            self.sole_healing_benefactor_expiration_frame,
        );
    }

    pub fn record_host_rebuild_producer(&self) {
        crate::game_logic::host_rebuild_producer_log::record(
            self.id,
            self.is_rebuild_hole,
            self.rebuild_template_name.clone().unwrap_or_default(),
            self.rebuild_ready_frame,
            self.rebuild_spawner_id.map(|id| id.0),
            self.rebuild_worker_id.map(|id| id.0),
            self.rebuild_reconstructing_id.map(|id| id.0),
            self.producer_id.map(|id| id.0),
            self.construction_complete_clear_frame,
        );
    }

    pub fn record_host_bounce_land(&self) {
        crate::game_logic::host_bounce_land_log::record(
            self.id,
            self.kill_when_resting_on_ground,
            self.bounce_land_events,
            self.last_bounce_fall_dy,
            self.bounce_sound_name.clone(),
            self.last_bounce_volume,
            self.bounce_audio_pending,
            self.allow_collide_force,
            self.last_collidee.map(|id| id.0),
            self.ignore_collisions_with.map(|id| id.0),
        );
    }

    pub fn record_host_physics_motive(&self) {
        crate::game_logic::host_physics_motive_log::record(
            self.id,
            self.motive_frames_remaining,
            self.physics_mass,
            [
                self.physics_accel.x,
                self.physics_accel.y,
                self.physics_accel.z,
            ],
            self.forward_friction,
            self.lateral_friction,
            self.z_friction,
            self.can_path_through_units,
            self.ignore_collisions_until_frame,
            self.is_panicking,
            self.move_away_frames,
            self.aerodynamic_friction,
            self.extra_friction,
            self.apply_friction_2d_when_airborne,
            self.center_of_mass_offset,
            self.pitch_roll_yaw_factor,
            self.move_away_destination.map(|p| [p.x, p.y, p.z]),
            self.request_other_move_away.map(|id| id.0),
            self.immune_to_falling_damage,
            self.physics_current_overlap.map(|id| id.0),
            self.physics_previous_overlap.map(|id| id.0),
        );
    }

    pub fn record_host_movement(&self) {
        crate::game_logic::host_movement_log::record(
            self.id,
            self.movement.velocity,
            self.movement.max_speed,
            self.movement.current_path_index,
            &self.movement.path,
            self.waiting_for_path,
            self.locomotor_surfaces,
            self.is_attack_path,
            self.is_blocked_and_stuck,
            self.is_braking,
            self.is_safe_path,
            self.queue_for_path_frames,
            self.path_timestamp,
            self.cur_max_blocked_speed,
            self.num_frames_blocked,
            self.is_blocked,
            self.move_away_from.map(|id| id.0),
            self.requested_victim_id.map(|id| id.0),
        );
        self.record_host_physics_motive();
    }

    pub fn record_host_weapon_stats(&self) {
        let (
            has_weapon,
            weapon_damage,
            weapon_range,
            weapon_min_range,
            weapon_reload_time,
            weapon_last_fire_time,
            weapon_clip_size,
            weapon_clip_reload_time,
            weapon_ammo,
            weapon_can_target_air,
            weapon_can_target_ground,
            weapon_projectile_speed,
        ) = if let Some(w) = self.weapon.as_ref() {
            (
                true,
                w.damage,
                w.range,
                w.min_range,
                w.reload_time,
                w.last_fire_time,
                w.clip_size,
                w.clip_reload_time,
                w.ammo.unwrap_or(u32::MAX),
                w.can_target_air,
                w.can_target_ground,
                w.projectile_speed,
            )
        } else {
            (
                false,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0,
                0.0,
                u32::MAX,
                false,
                true,
                0.0,
            )
        };
        let (has_secondary_weapon, secondary_weapon_damage, secondary_weapon_range) =
            if let Some(w) = self.secondary_weapon.as_ref() {
                (true, w.damage, w.range)
            } else {
                (false, 0.0, 0.0)
            };
        crate::game_logic::host_weapon_stats_log::record(
            crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent {
                object: self.id,
                has_weapon,
                weapon_damage,
                weapon_range,
                weapon_min_range,
                weapon_reload_time,
                weapon_last_fire_time,
                weapon_clip_size,
                weapon_clip_reload_time,
                weapon_ammo,
                weapon_can_target_air,
                weapon_can_target_ground,
                weapon_projectile_speed,
                has_secondary_weapon,
                secondary_weapon_damage,
                secondary_weapon_range,
                leech_range_active_primary: self.leech_range_active_primary,
                leech_range_active_secondary: self.leech_range_active_secondary,
            },
        );
    }

    pub fn record_host_vision_camo(&self) {
        crate::game_logic::host_vision_camo_log::record(
            self.id,
            self.vision_spied_mask,
            self.camo_friendly_opacity,
            self.camo_stealth_look,
        );
    }

    pub fn record_host_command_set(&self) {
        crate::game_logic::host_command_set_log::record(self.id, self.command_set_override.clone());
    }

    pub fn set_command_set_override(&mut self, command_set: Option<String>) {
        if self.command_set_override != command_set {
            self.command_set_override = command_set;
            self.record_host_command_set();
        }
    }

    pub fn record_host_disguise(&self) {
        let team = self
            .disguise_as_team
            .map(|t| match t {
                Team::USA => 0,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 3,
            })
            .unwrap_or(255);
        crate::game_logic::host_disguise_log::record(
            self.id,
            self.disguise_as_template.clone(),
            team,
        );
    }

    pub fn record_host_overlord(&self) {
        let bunker_capacity = match self.overlord_bunker_capacity {
            Some(n) => n.min(u16::MAX as usize - 1) as u16,
            None => u16::MAX,
        };
        crate::game_logic::host_overlord_log::record(
            self.id,
            self.has_overlord_gattling_addon,
            self.has_overlord_propaganda_addon,
            bunker_capacity,
            self.is_helix_transport,
        );
    }

    pub fn record_host_stealth_flags(&self) {
        crate::game_logic::host_stealth_flags_log::record(
            crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent {
                object: self.id,
                innate_stealth: self.innate_stealth,
                stealth_breaks_on_attack: self.stealth_breaks_on_attack,
                stealth_breaks_on_move: self.stealth_breaks_on_move,
                is_tunnel_network: self.is_tunnel_network,
                passengers_allowed_to_fire: self.passengers_allowed_to_fire,
            },
        );
    }

    pub fn record_host_hive(&self) {
        crate::game_logic::host_hive_log::record(
            self.id,
            self.hive_slave_count,
            self.hive_slave_hp,
        );
    }

    pub fn record_host_contain_capacity(&self) {
        let max_garrison = self
            .building_data
            .as_ref()
            .map(|bd| bd.max_garrison.min(u16::MAX as usize) as u16)
            .unwrap_or(0);
        crate::game_logic::host_contain_capacity_log::record(
            self.id,
            self.max_transport,
            max_garrison,
        );
    }

    pub fn record_host_overcharge(&self) {
        crate::game_logic::host_overcharge_log::record(self.id, self.overcharge_enabled);
    }

    pub fn set_overcharge_enabled(&mut self, enabled: bool) {
        if self.overcharge_enabled != enabled {
            self.overcharge_enabled = enabled;
            self.record_host_overcharge();
        }
    }

    /// C++ Object::setWeaponSetFlag(WEAPONSET_MINE_CLEARING_DETAIL) residual.

    /// C++ SpecialPowerUpdateInterface::setSpecialPowerOverridableDestination residual.
    pub fn set_special_power_overridable_destination(
        &mut self,
        loc: Vec3,
        power: Option<crate::command_system::SpecialPowerType>,
    ) {
        self.special_power_override_destination = Some(loc);
        self.special_power_override_type = power;
    }

    pub fn clear_special_power_overridable_destination(&mut self) {
        self.special_power_override_destination = None;
        self.special_power_override_type = None;
    }

    /// C++ Object::setWeaponSetFlag residual (subset used by AIGroup).
    /// `flag`: 0=PLAYER_UPGRADE, 1=MINE_CLEARING, 2=CARBOMB, 3=VEHICLE_HIJACK.
    pub fn set_weapon_set_flag(&mut self, flag: u8, enabled: bool) -> bool {
        let previous_sources = self.active_weapon_barrel_source_identities();
        let previous = match flag {
            0 => self.weapon_set_player_upgrade,
            1 => self.weapon_set_mine_clearing_detail,
            2 => self.weapon_set_carbomb,
            3 => self.weapon_set_vehicle_hijack,
            _ => return false,
        };
        match flag {
            0 => self.weapon_set_player_upgrade = enabled,
            1 => self.weapon_set_mine_clearing_detail = enabled,
            2 => self.weapon_set_carbomb = enabled,
            3 => self.weapon_set_vehicle_hijack = enabled,
            _ => return false,
        }
        if previous != enabled {
            // C++ setWeaponSetFlag / clearWeaponSetFlag → updateWeaponSet.
            let condition = match flag {
                0 => "PLAYER_UPGRADE",
                1 => "MINE_CLEARING_DETAIL",
                2 => "CARBOMB",
                3 => "VEHICLE_HIJACK",
                _ => "",
            };
            if enabled {
                self.adopt_weapon_set_lock_share_for_condition(condition);
            }
            self.release_weapon_lock_on_set_change();
        }
        self.reset_weapon_barrel_states_if_sources_changed(previous_sources);
        self.record_host_weapon_set();
        true
    }

    pub fn set_weapon_set_mine_clearing_detail(&mut self, enabled: bool) {
        // C++ WorkerAIUpdate.cpp:1002-1014 drops carried boxes only from the
        // aiDoCommand tail when `isClearingMines() && m_numberBoxes > 0` —
        // arming the WEAPONSET_MINE_CLEARING detail itself never spends them.
        // host drop_worker_supply_boxes_for_mine_clear owns the guarded drop.
        let _ = self.set_weapon_set_flag(1, enabled);
    }

    /// C++ AICMD_GO_PRONE residual — infantry hit the dirt briefly.
    pub fn go_prone(&mut self, duration_secs: f32) {
        self.stop_moving();
        self.set_target(None);
        self.set_force_attack(false);
        self.prone_timer = duration_secs.max(0.1);
        if let Some(pu) = self.prone_update.as_mut() {
            // Approximate seconds → frames at 30 Hz for module residual.
            let frames = (duration_secs.max(0.1) * 30.0).round() as i32;
            let was = pu.prone_frames > 0;
            pu.prone_frames = pu.prone_frames.max(frames);
            if !was {
                pu.model_prone = true;
                pu.no_attack = true;
            }
        }
        if let Some(bit) =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("PRONE")
        {
            self.model_condition_bits |= 1u128 << bit;
            // Wave 487: prone model bit must reach GW before model-condition writeback.
            self.record_host_model_condition();
        }
        // Stay in Idle while prone so orders can break it; timer clears the bit.
        if !matches!(
            self.ai_state,
            AIState::Attacking
                | AIState::AttackMoving
                | AIState::GuardingArea
                | AIState::GuardingObject
        ) {
            self.set_ai_state(AIState::Idle);
        }
        self.record_host_locomotor();
    }

    pub fn record_host_weapon_set(&self) {
        crate::game_logic::host_weapon_set_log::record(
            self.id,
            self.weapon_set_player_upgrade,
            self.armed_riders_upgrade_weapon_set,
        );
    }

    pub fn record_host_ai_attitude(&self) {
        crate::game_logic::host_ai_attitude_log::record(self.id, self.ai_attitude);
    }

    pub fn set_ai_attitude_i8(&mut self, attitude: i8) {
        let a = attitude.clamp(-2, 2);
        if self.ai_attitude != a {
            self.ai_attitude = a;
            self.record_host_ai_attitude();
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if self.status.destroyed {
            return;
        }
        // C++ PoisonedBehavior::onHealing residual.
        self.clear_poisoned_on_healing();
        let before = self.health.current;
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let projected = (before + amount).min(self.health.maximum);
        if projected <= before {
            return;
        }
        // GameWorld HP authority: log absolute health; defer host mutate to writeback.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_heal_log::record(self.id, projected);
        } else {
            self.health.heal(amount);
            crate::game_logic::host_heal_log::record(self.id, self.health.current);
        }
        self.refresh_model_condition_bits();
    }
}
