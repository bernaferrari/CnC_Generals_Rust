//! Receiving AIUpdate virtual aiIdle(CMD_FROM_AI), distinct from AIGroup::groupIdle.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId};
use gamelogic::{GameLogicError, GameLogicResult};

impl GameLogic {
    pub(crate) fn unit_command_ai_idle_owned(&mut self, id: ObjectId) -> GameLogicResult<bool> {
        self.unit_command_ai_idle_owned_with_effects(id, &mut Default::default())
    }

    pub(crate) fn unit_command_ai_idle_owned_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        let Some(unit) = self.objects.get(&id) else {
            return Ok(false);
        };
        if !unit.has_ai_update_interface() {
            return Ok(false);
        }
        // Immutable installed module identity, not object name/KindOf guesswork.
        let module = unit
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|template| template.receiving_ai_module_name())
            .ok_or_else(|| {
                GameLogicError::Configuration(
                    "AI command lacks admitted first source identity".into(),
                )
            })?
            .to_ascii_lowercase();
        let birth = std::sync::Arc::clone(&unit.visual_draw_identity_token);

        // FlightDeckBehavior.cpp1356: source AI skips the entire command switch.
        if module == "flightdeckbehavior" {
            return Ok(true);
        }
        // Derived pre-gate work is observable even when the command is refused.
        if matches!(module.as_str(), "dozeraiupdate" | "workeraiupdate") {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_actively_constructing(false);
            }
        }
        if module == "jetaiupdate" {
            self.receiving_jet_producer_location_owned(id);
        }
        if module == "chinookaiupdate" {
            let previous = self
                .objects
                .get(&id)
                .and_then(|unit| unit.chinook_ai.as_ref())
                .and_then(|ai| ai.airfield_id);
            if let Some(previous) = previous {
                self.set_airfield_healee(ObjectId(previous), id, false);
            }
            if let Some(ai) = self
                .objects
                .get_mut(&id)
                .and_then(|unit| unit.chinook_ai.as_mut())
            {
                ai.airfield_id = None;
                ai.healee = false;
            }
        }
        let Some(unit) = self.objects.get(&id) else {
            return Ok(false);
        };
        // AIUpdate.cpp2572/4287: health zero is not the effectively-dead bit;
        // remote human controllers remain human, and absent controller has no mood.
        let sleep = unit.ai_attitude()
            == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
            && unit
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .is_some_and(|player| !player.is_human);
        if unit.status.effectively_dead || sleep {
            return Ok(true);
        }

        if module == "hackinternetaiupdate" {
            if self.hacker_income.replace_packing_command(
                id,
                crate::game_logic::host_hacker_income::PendingHackerCommand::AiIdle,
            ) {
                return Ok(true);
            }
            let was_hacking = self.hacker_income.pack_phase(id)
                == Some(crate::game_logic::host_hacker_income::HackerInternetPhase::Hacking);
            if self.note_hacker_ai_command(
                id,
                crate::game_logic::host_hacker_income::PendingHackerCommand::AiIdle,
            ) {
                if was_hacking {
                    if let Some(unit) = self.objects.get_mut(&id) {
                        unit.last_command_source =
                            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                    }
                }
                return Ok(true);
            }
        }
        if module == "jetaiupdate" {
            let Some(unit) = self.objects.get_mut(&id) else {
                return Ok(false);
            };
            // JetAIUpdate.cpp2390 always stores the most recent command.
            unit.jet_ai.pending_resume = crate::game_logic::object::HostJetPendingResume::AiIdle;
            if unit.jet_ai.takeoff_in_progress
                || unit.jet_ai.landing_in_progress
                || unit.airfield_rearm_ready_frame.is_some()
            {
                unit.jet_ai.has_pending_command = true;
                return Ok(true);
            }
            if unit.status.airborne_target && !Self::object_is_produced_at_helipad(unit) {
                // This derived branch returns before base Idle and passenger recursion.
                return self.clear_base_ai_for_jet_return_owned(id, effects);
            }
            unit.jet_ai.has_pending_command = false;
            unit.jet_ai.allow_interrupt_for_reload = false;
        }
        if module == "chinookaiupdate" {
            if let Some(ai) = self
                .objects
                .get_mut(&id)
                .and_then(|unit| unit.chinook_ai.as_mut())
            {
                if matches!(ai.flight_status,
                    crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::DoingCombatDrop) {
                    ai.pending_evac_dest = None;
                    ai.pending_evac_and_exit = false;
                    ai.pending_ai_idle = true;
                    return Ok(true);
                }
                ai.pending_ai_idle = false;
            }
        }
        // Assault/Dozer/Worker/Supply/Railed/Deploy/POW source-AI Idle reaches
        // inherited privateIdle without their player-only resets/cancellation.
        // Only that inherited branch recursively calls the passengers' *derived* AI.
        if self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.is_kind_of(KindOf::Projectile))
        {
            return Ok(true);
        }
        if !self.unit_command_standard_ai_idle_tail_owned(id, effects)? {
            return Ok(false);
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
        }
        let occupants = self
            .objects
            .get(&id)
            .filter(|unit| std::sync::Arc::ptr_eq(&birth, &unit.visual_draw_identity_token))
            .map(|unit| unit.contained_units())
            .unwrap_or_default();
        for occupant in occupants {
            if occupant != id {
                self.unit_command_ai_idle_owned_with_effects(occupant, effects)?;
                if !self.objects.get(&id).is_some_and(|unit| {
                    std::sync::Arc::ptr_eq(&birth, &unit.visual_draw_identity_token)
                }) {
                    return Err(GameLogicError::Configuration(
                        "Idle passenger callback lost its receiving parent".into(),
                    ));
                }
            }
        }
        Ok(true)
    }
}
