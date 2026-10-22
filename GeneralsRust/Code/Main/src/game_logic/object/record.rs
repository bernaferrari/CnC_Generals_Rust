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

    pub fn set_ground_height_residual(&mut self, height: f32, from_terrain: bool) {
        let changed = (self.ground_height - height).abs() > f32::EPSILON
            || self.ground_height_from_terrain != from_terrain;
        if !changed {
            return;
        }
        self.ground_height = height;
        self.ground_height_from_terrain = from_terrain;
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

    /// C++ CrushDie model condition FRONTCRUSHED/BACKCRUSHED residual.
    pub fn apply_crush_die_model_conditions(&mut self) {
        use crate::game_logic::host_neutron_missile_slow_death::{
            MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
        };
        // Clear then set like C++ clearAndSetModelConditionFlags.
        self.model_condition_bits &= !(1u128 << MC_BIT_FRONTCRUSHED);
        self.model_condition_bits &= !(1u128 << MC_BIT_BACKCRUSHED);
        if self.front_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_FRONTCRUSHED;
        }
        if self.back_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_BACKCRUSHED;
        }
    }

    pub fn set_formation(&mut self, formation_id: u32, formation_offset: glam::Vec2) {
        self.formation_id = formation_id;
        self.formation_offset = formation_offset;
    }

    pub fn begin_cheer(
        &mut self,
        duration_secs: f32,
        cheer_bit: Option<usize>,
        audio: &mut crate::game_logic::MovementAudio<'_>,
    ) {
        self.set_ai_state(AIState::SpecialAbility, audio);
        self.cheer_timer = duration_secs.max(0.0);
        if let Some(bit) = cheer_bit {
            if bit < 128 {
                self.model_condition_bits |= 1u128 << bit;
            }
        }
    }

    pub fn set_selection_radius(&mut self, selection_radius: f32) {
        if (self.selection_radius - selection_radius).abs() > f32::EPSILON {
            self.selection_radius = selection_radius;
        }
    }

    pub fn set_command_set_override(&mut self, command_set: Option<String>) {
        if self.command_set_override != command_set {
            self.command_set_override = command_set;
        }
    }

    pub fn set_overcharge_enabled(&mut self, enabled: bool) {
        if self.overcharge_enabled != enabled {
            self.overcharge_enabled = enabled;
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
            1 => return false,
            2 => self.weapon_set_carbomb,
            3 => self.weapon_set_vehicle_hijack,
            _ => return false,
        };
        match flag {
            0 => self.weapon_set_player_upgrade = enabled,

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
        true
    }

    /// C++ AICMD_GO_PRONE residual — infantry hit the dirt briefly.
    pub fn go_prone(
        &mut self,
        duration_secs: f32,
        audio: &mut crate::game_logic::MovementAudio<'_>,
    ) {
        self.clear_target(audio);
        self.set_force_attack(false);
        self.stop_moving(audio);
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
        }
        // Stay in Idle while prone so orders can break it; timer clears the bit.
        if !matches!(
            self.ai_state,
            AIState::Attacking
                | AIState::AttackMoving
                | AIState::GuardingArea
                | AIState::GuardingObject
        ) {
            self.set_ai_state(AIState::Idle, audio);
        }
    }

    pub fn set_ai_attitude_i8(&mut self, attitude: i8) {
        let a = attitude.clamp(-2, 2);
        if self.ai_attitude != a {
            self.ai_attitude = a;
        }
    }

    pub fn heal(
        &mut self,
        amount: f32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        frame: u32,
    ) {
        self.heal_with_source(amount, None, health_events, frame);
    }

    pub(super) fn heal_with_source(
        &mut self,
        amount: f32,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        frame: u32,
    ) {
        if self.status.destroyed || self.is_inactive_body() {
            return;
        }
        let before = self.health.current;
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let cap = if self.health.maximum > 0.0 {
            self.health.maximum
        } else {
            self.max_health
        };
        let projected = (before + amount).min(cap);
        if projected <= before {
            if projected == before {
                self.previous_health = before;
            }
            return;
        }
        // C++ ActiveBody::attemptHealing -> internalChangeHealth commits the
        // body before subsequent callbacks/readers. Shadow admission observes
        // this completed write; it must not decide when the owner sees healing.
        if self.health.maximum <= 0.0 && cap > 0.0 {
            self.health.maximum = cap;
        }
        let old_body_state = self.internal_change_health(projected - before);
        health_events.record_applied_heal(self.id, self.health.current);
        // CPP onHealing callbacks run after ordinary Body math/flags.
        self.clear_poisoned_on_healing();
        self.react_to_body_damage_state_change(old_body_state, source, frame);
    }
}
