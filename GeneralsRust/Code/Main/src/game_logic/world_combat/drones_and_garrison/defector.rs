use super::super::super::*;

impl GameLogic {
    /// Residual honesty: Baikonur launch door and/or detonation recorded.

    pub fn honesty_defector_ok(&self) -> bool {
        self.defector_special.honesty_ok()
    }

    /// C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
    /// ActionManager.cpp:1696-1710 rejects STRUCTURE and non-ENEMIES;
    /// Object.cpp:6111-6220 `defect` after those guards.
    #[cfg(test)]
    pub fn activate_defector(&mut self, caster_id: ObjectId, victim_id: ObjectId) -> bool {
        self.activate_defector_with_effects(
            caster_id,
            victim_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("valid defector test fixture")
    }

    pub(crate) fn activate_defector_with_effects(
        &mut self,
        caster_id: ObjectId,
        victim_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        use crate::game_logic::host_defector_special_power::DEFECTOR_DETECTION_FRAMES;
        if caster_id == victim_id {
            return Ok(false);
        }
        let Some(caster) = self.objects.get(&caster_id) else {
            return Ok(false);
        };
        if caster.is_disabled() {
            return Ok(false);
        }
        let Some(destination) = caster.team_instance_id else {
            return Ok(false);
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return Ok(false);
        };
        if !victim.is_alive() || victim.is_kind_of(KindOf::Structure) {
            return Ok(false);
        }
        let Some(source) = victim.team_instance_id else {
            return Ok(false);
        };
        // ActionManager.cpp:1696–1710 requires the actual team relationship.
        if self.host_team_relationship(destination, source)
            != Some(gamelogic::common::Relationship::Enemies)
        {
            return Ok(false);
        }
        if !self.defect_host_object_with_effects(
            victim_id,
            destination,
            DEFECTOR_DETECTION_FRAMES,
            presentation_effects,
        )? {
            return Ok(false);
        }
        self.defector_special
            .record(victim_id.0, DEFECTOR_DETECTION_FRAMES);
        Ok(true)
    }
}
