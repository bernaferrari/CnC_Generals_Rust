use super::super::super::*;

impl GameLogic {
    /// Residual honesty: Baikonur launch door and/or detonation recorded.

    pub fn honesty_defector_ok(&self) -> bool {
        self.defector_special.honesty_ok()
    }

    /// C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
    /// ActionManager.cpp:1696-1710 rejects STRUCTURE and non-ENEMIES;
    /// Object.cpp:6111-6220 `defect` after those guards.
    pub fn activate_defector(&mut self, caster_id: ObjectId, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_defector_special_power::{
            DEFECTOR_DETECTION_FRAMES, DEFECTOR_TIMER_TICK_AUDIO, resolve_voice_defect,
        };
        if caster_id == victim_id {
            return false;
        }
        let Some(caster) = self.objects.get(&caster_id) else {
            return false;
        };
        if caster.is_disabled() {
            return false;
        }
        let caster_team = caster.team;
        if caster_team == Team::Neutral {
            return false;
        }
        let caster_owner = self.player_owner_for_host_object(caster);
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if !victim.is_alive() {
            return false;
        }
        if victim.is_kind_of(KindOf::Structure) {
            return false;
        }
        // C++ relationship ENEMIES only (neutral / same-team are worthless).
        if victim.team == caster_team || victim.team == Team::Neutral {
            return false;
        }
        if victim.contained_by.is_some() {
            return false;
        }
        if victim.status.under_construction
            || victim.construction_percent + 0.001 < 1.0
            || victim.status.sold
        {
            return false;
        }
        let old_team = victim.team;
        let old_owner = self.player_owner_for_host_object(victim);
        let victim_pos = victim.get_position();
        let victim_template = victim.template_name.clone();
        let frames = DEFECTOR_DETECTION_FRAMES;
        let now = self.frame;

        // C++ Object::defect before switch: refund production, radar ping.
        self.cancel_all_production(victim_id);
        let old_playable = old_owner
            .map(|id| self.player_is_playable_side(id))
            .unwrap_or(false);
        let new_playable = caster_owner
            .map(|id| self.player_is_playable_side(id))
            .unwrap_or(caster_team != Team::Neutral);
        if old_playable && new_playable {
            self.try_infiltration_event(victim_id);
        }

        let Some(victim) = self.objects.get_mut(&victim_id) else {
            return false;
        };
        victim.set_team_and_owner(caster_team, caster_owner);
        victim.begin_undetected_defection(now, frames, true);

        // C++ after switch: handlePartitionCellMaintenance + aiIdle.
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            victim.stop_moving();
            victim.set_status_moving(false);
            victim.set_status_attacking(false);
            victim.set_target(None);
            victim.set_ai_state(AIState::Idle);
            victim.flash_as_selected();
        }
        self.stop_attack_decision_aware(victim_id);
        self.clear_target_decision_aware(victim_id);

        // C++ `*getTemplate()->getVoiceDefect()` + defector timer tick.
        // Missing VoiceDefect is an empty AudioEventRTS (silent), never the slot token.
        if let Some(event) = resolve_voice_defect(&victim_template) {
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(victim_id)
                    .with_position(victim_pos)
                    .with_priority(180),
            );
        }
        self.queue_audio_event(
            AudioEventRequest::new(DEFECTOR_TIMER_TICK_AUDIO)
                .with_object(victim_id)
                .with_position(victim_pos)
                .with_priority(160),
        );

        // C++ kickOutOnCapture removeAllContained (tunnels/caves skip).
        self.on_capture_kick_passengers(victim_id, old_team, caster_team);

        // C++ ParkingPlaceBehavior::defectAllParkedUnits.
        self.defect_all_parked_units(victim_id);

        // C++ world walk: KINDOF_MINE whose producer is this object setTeam.
        let mine_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_kind_of(KindOf::Mine) && o.producer_id == Some(victim_id))
            .map(|(id, _)| *id)
            .collect();
        for mine_id in mine_ids {
            if let Some(mine) = self.objects.get_mut(&mine_id) {
                mine.set_team_and_owner(caster_team, caster_owner);
            }
        }

        self.defector_special.record(victim_id.0 as u32, frames);
        true
    }

    /// C++ SpecialPowerModule ctor path: StartsPaused → pauseCountdown(TRUE).

    /// C++ SupplyWarehouseCreate::onCreate residual.

    /// C++ SpecialPowerCompletionDie::onDie → notifyOfCompletedSpecialPower residual.
    pub(crate) fn maybe_notify_special_power_completion(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let Some(ref data) = obj.special_power_completion else {
            return;
        };
        if !data.creator_set {
            return;
        }
        let power = data.special_power_name.clone();
        let creator = data.creator_id;
        let team = obj.team;
        let player_id = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.id)
            .unwrap_or(0);
        crate::game_logic::script_events::push_event(
            crate::game_logic::script_events::ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name: power.clone(),
                creator_id: creator,
            },
        );
        self.special_power_completion_log
            .record_notify(&power, creator);
    }
}
