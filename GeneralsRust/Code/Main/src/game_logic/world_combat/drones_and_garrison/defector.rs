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
        use crate::game_logic::host_defector_special_power::DEFECTOR_DETECTION_FRAMES;
        if caster_id == victim_id {
            return false;
        }
        let Some(caster) = self.objects.get(&caster_id) else {
            return false;
        };
        if caster.is_disabled() {
            return false;
        }
        let Some(destination) = caster.team_instance_id else {
            return false;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if !victim.is_alive() || victim.is_kind_of(KindOf::Structure) {
            return false;
        }
        let Some(source) = victim.team_instance_id else {
            return false;
        };
        // ActionManager.cpp:1696–1710 requires the actual team relationship.
        if self.host_team_relationship(destination, source)
            != Some(gamelogic::common::Relationship::Enemies)
        {
            return false;
        }
        if !self.defect_host_object(victim_id, destination, DEFECTOR_DETECTION_FRAMES) {
            return false;
        }
        self.defector_special
            .record(victim_id.0, DEFECTOR_DETECTION_FRAMES);
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
