//! Host combat `impl GameLogic` — `special_power_flights`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

mod anthrax_cluster_emp_flights;
mod markers_and_pings;
mod tunnel_cave_bridge;
mod chinook_and_payloads;
#[cfg(test)]
mod bridge_ocl_receiving_owner_tests;
#[cfg(test)]
mod initial_transport_payload_owner_tests;
#[cfg(test)]
mod initial_transport_disabled_mask_tests;

impl GameLogic {
    pub fn honesty_carpet_bomb_flight_ok(&self) -> bool {
        crate::game_logic::host_carpet_bomb_flight::honesty_carpet_bomb_flight_residual_ok()
    }

    pub fn honesty_artillery_barrage_flight_ok(&self) -> bool {
        crate::game_logic::host_artillery_barrage_flight::honesty_artillery_barrage_flight_residual_ok()
    }

    pub fn honesty_a10_strike_flight_ok(&self) -> bool {
        crate::game_logic::host_a10_strike_flight::honesty_a10_strike_flight_residual_ok()
    }

    pub fn honesty_daisy_cutter_flight_ok(&self) -> bool {
        crate::game_logic::host_daisy_cutter_flight::honesty_daisy_cutter_flight_residual_ok()
    }

    pub fn honesty_anthrax_bomb_flight_ok(&self) -> bool {
        crate::game_logic::host_anthrax_bomb_flight::honesty_anthrax_bomb_flight_residual_ok()
    }

    pub fn honesty_cluster_mines_flight_ok(&self) -> bool {
        crate::game_logic::host_cluster_mines_flight::honesty_cluster_mines_flight_residual_ok()
    }

    pub fn honesty_emp_pulse_flight_ok(&self) -> bool {
        crate::game_logic::host_emp_pulse_flight::honesty_emp_pulse_flight_residual_ok()
    }

    pub fn honesty_scud_storm_missile_flight_ok(&self) -> bool {
        crate::game_logic::host_scud_storm_missile_flight::honesty_scud_storm_missile_flight_residual_ok()
    }

    /// Residual honesty: Neutron shell ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_neutron_shell_scatter_ok(&self) -> bool {
        self.neutron_shell_scatter_applied > 0 || self.neutron_shell_scatter_misses > 0
    }

    pub fn honesty_neutron_missile_update_ok(&self) -> bool {
        crate::game_logic::host_neutron_missile_update::honesty_neutron_missile_update_residual_ok()
    }

    pub fn honesty_ocl_apply_random_force_ok(&self) -> bool {
        crate::game_logic::host_ocl_apply_random_force::honesty_ocl_apply_random_force_residual_ok()
    }

    pub fn honesty_fuel_air_gas_slow_death_ok(&self) -> bool {
        crate::game_logic::host_fuel_air_gas_slow_death::honesty_fuel_air_gas_slow_death_residual_ok(
        )
    }

    pub fn honesty_ocl_fire_weapon_attack_ok(&self) -> bool {
        crate::game_logic::host_ocl_fire_weapon_attack::honesty_ocl_fire_weapon_attack_residual_ok()
    }

    pub fn honesty_ocl_create_debris_ok(&self) -> bool {
        crate::game_logic::host_ocl_create_debris::honesty_ocl_create_debris_residual_ok()
            && (self.ocl_create_debris_reg.plans > 0
                || self.ocl_create_debris_reg.debris_spawned == 0)
    }

    pub fn honesty_ocl_special_power_ok(&self) -> bool {
        self.ocl_special_power_reg.honesty_host_path_ok()
            && crate::game_logic::host_ocl_special_power::honesty_ocl_special_power_residual_ok()
    }

    pub fn tensile_formation_registry(
        &self,
    ) -> &crate::game_logic::host_tensile_formation::HostTensileFormationRegistry {
        &self.tensile_formation_reg
    }

    pub fn honesty_highlander_body_ok(&self) -> bool {
        self.highlander_body_reg.honesty_clamp_ok()
    }

    pub fn honesty_upgrade_die_ok(&self) -> bool {
        self.upgrade_die_reg.honesty_removal_ok()
    }

    pub fn record_highlander_clamp(&mut self) {
        self.highlander_body_reg.record_clamp();
    }

    /// Residual honesty: Battle Bus passenger residual fire.
    pub fn honesty_battle_bus_passenger_fire_ok(&self) -> bool {
        self.battle_bus.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Battle Bus armed-riders weapon-set upgrade.
    pub fn honesty_battle_bus_weapon_set_upgrade_ok(&self) -> bool {
        self.battle_bus.honesty_weapon_set_upgrade_ok()
    }

    /// Record a residual Overlord BattleBunker enter (tests / host path).
    pub fn record_overlord_bunker_residual_enter(&mut self) {
        self.overlord_bunker_residual_enters =
            self.overlord_bunker_residual_enters.saturating_add(1);
    }

    /// Record a residual Overlord BattleBunker exit/evacuate (tests / host path).
    pub fn record_overlord_bunker_residual_exit(&mut self) {
        self.overlord_bunker_residual_exits = self.overlord_bunker_residual_exits.saturating_add(1);
    }
}

impl GameLogic {
    /// TransportContain.cpp:457-459 creates InitialPayload before its ordinary
    /// transport update. An empty passenger list is never a birth/retry signal.
    pub(crate) fn update_initial_transport_payloads_with_effects(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // GameLogic::registerObject clamps a constructor's first wake to1.
        if self.frame == 0 {
            return Ok(());
        }
        // GameLogic.cpp3721-3723 and UpdateModule.h154-157: this module
        // processes DISABLEDMASK_NONE, so any disabled bit defers its update.
        // Dying/zero-health and UNDER_CONSTRUCTION are separate source state.
        let carriers: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(id, object)| {
                (!object.has_disabled_type()
                    && !object.initial_transport_payload_created
                    && (object.is_listening_outpost_transport
                        || object.is_troop_crawler_transport
                        || object.thing().template.contain_module.kind
                            == crate::game_logic::ContainModuleKind::RiderChange))
                    .then_some((*id, object.visual_object_generation))
            })
            .collect();
        for (id, generation) in carriers {
            let Some((team, position, listening, crawler, rider_payload, owner)) = self
                .objects
                .get(&id)
                .filter(|object| {
                    !object.has_disabled_type()
                        && object.visual_object_generation == generation
                        && !object.initial_transport_payload_created
                })
                .map(|object| {
                    (
                        object.team,
                        object.get_position(),
                        object.is_listening_outpost_transport,
                        object.is_troop_crawler_transport,
                        object.thing().template.contain_module.initial_transport_payload.clone(),
                        object.owner_player_id,
                    )
                })
            else {
                continue;
            };
            if self.objects[&id].thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RiderChange
            {
                // C++ resolves RiderN through the session's ThingFactory.
                let roster: Vec<_> = self.objects[&id].thing().template.contain_module
                    .rider_change_riders.iter().map(|row| row.template_name.clone()).collect();
                for template in roster {
                    self.ensure_host_spawn_template(&template);
                }
            }
            if listening {
                self.apply_listening_outpost_initial_payload_with_effects_checked(
                    id, team, position, None, ui,
                )?;
            } else if crawler {
                self.apply_troop_crawler_initial_payload_with_effects_checked(
                    id, team, position, None, ui,
                )?;
            } else {
                // Keep the latch false throughout callbacks: RiderChange's
                // onContaining suppresses evacuation until createPayload ends.
                let result = (|| -> gamelogic::GameLogicResult<()> {
                    if let Some((name, count)) = rider_payload {
                        for _ in 0..count.max(0) {
                            let Some(rider_id) = self.create_object_for_owner_or_team_with_effects_checked(
                                &name, team, owner, glam::Vec3::ZERO, None, ui,
                            )? else {
                                continue;
                            };
                            if !self.objects.get(&id).is_some_and(|object| {
                                object.visual_object_generation == generation
                            }) {
                                break;
                            }
                            // Invalid riders remain real, uncontained objects,
                            // just as the source's failed admission does.
                            let _ = self.rider_change_initial_payload_with_effects(rider_id, id, ui)?;
                        }
                    }
                    Ok(())
                })();
                if let Some(object) = self.objects.get_mut(&id)
                    .filter(|object| object.visual_object_generation == generation)
                {
                    object.initial_transport_payload_created = true;
                }
                result?;
                continue;
            }
            // Original createPayload sets the latch after admission. A failed
            // semantic child birth propagates instead of pretending success.
            if let Some(object) = self
                .objects
                .get_mut(&id)
                .filter(|object| object.visual_object_generation == generation)
            {
                object.initial_transport_payload_created = true;
            }
        }
        Ok(())
    }
}
