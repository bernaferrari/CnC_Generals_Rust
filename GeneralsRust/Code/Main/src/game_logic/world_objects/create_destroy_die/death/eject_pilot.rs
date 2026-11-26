//! EjectPilotDie selects the receiving creation list and submits the source's cues.
use crate::game_logic::{
    ContainModuleKind, EjectPilotCreationList, EjectPilotDeathTypes, EjectPilotExemptStatus,
    EjectPilotVeterancyLevels, GameLogic, ObjectId, ScriptPresentationEffects, VeterancyLevel,
};

impl GameLogic {
    /// Adapter for template-only death declarations and callers without a Body hit.
    pub(crate) fn maybe_apply_eject_pilot_die_with_effects(
        &mut self,
        id: ObjectId,
        damage_dealer: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        use crate::game_logic::host_usa_pilot::HostDeathType;

        let (metadata, veterancy, death_type, is_hijacked) = {
            let Some(obj) = self.objects.get(&id) else {
                return Ok(());
            };
            if obj.eject_pilot_die_applied {
                return Ok(());
            }
            let Some(metadata) = obj.thing().template.eject_pilot_die else {
                // Module presence, not an object basename, is the C++ die
                // authority.  A name-shaped vehicle with no parsed module is
                // intentionally inert here.
                return Ok(());
            };
            (
                metadata,
                obj.experience.level,
                obj.status.death_type,
                obj.status.hijacked,
            )
        };

        // C++ invokes a DieModule once per death.  The host may visit this
        // object again while SlowDeath unwinds, so record the attempt before
        // any supported filter/OCL can decline it.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.eject_pilot_die_applied = true;
        }

        let death_is_crushed_or_splatted =
            matches!(death_type, HostDeathType::Crushed | HostDeathType::Splatted);
        let veterancy_is_regular = matches!(veterancy, VeterancyLevel::Rookie);
        let death_types_gate = match metadata.death_types {
            EjectPilotDeathTypes::All => true,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted => !death_is_crushed_or_splatted,
            EjectPilotDeathTypes::Unsupported => false,
        };
        let veterancy_gate = match metadata.veterancy_levels {
            EjectPilotVeterancyLevels::All => true,
            EjectPilotVeterancyLevels::AllExceptRegular => !veterancy_is_regular,
            EjectPilotVeterancyLevels::Unsupported => false,
        };
        let exempt_status_gate = match metadata.exempt_status {
            EjectPilotExemptStatus::None => true,
            EjectPilotExemptStatus::Hijacked => !is_hijacked,
            EjectPilotExemptStatus::Unsupported => false,
        };

        // Preserve the existing observability counters, but now only when the
        // corresponding parsed DieMux clause actually owns the block.
        if matches!(
            metadata.veterancy_levels,
            EjectPilotVeterancyLevels::AllExceptRegular
        ) && death_types_gate
            && exempt_status_gate
            && !veterancy_gate
        {
            self.usa_pilot.record_eject_veterancy_block();
        }
        if matches!(
            metadata.death_types,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted
        ) && veterancy_gate
            && exempt_status_gate
            && !death_types_gate
        {
            self.usa_pilot.record_eject_death_type_block();
        }
        if matches!(metadata.exempt_status, EjectPilotExemptStatus::Hijacked)
            && veterancy_gate
            && death_types_gate
            && !exempt_status_gate
        {
            self.usa_pilot.record_eject_hijacked_block();
        }

        if !metadata.allows_supported_death(
            death_is_crushed_or_splatted,
            veterancy_is_regular,
            is_hijacked,
        ) {
            return Ok(());
        }

        let name = |selection| match selection {
            EjectPilotCreationList::OnGround => "OCL_EjectPilotOnGround",
            EjectPilotCreationList::ViaParachute => "OCL_EjectPilotViaParachute",
        };
        self.run_receiving_eject_pilot_with_effects(
            id,
            metadata.ground_creation_list.map(name),
            metadata.air_creation_list.map(name),
            damage_dealer,
            interpreter,
            presentation_effects,
        )
    }

    /// EjectPilotDie.cpp:65–89. The selected list owns Object creation,
    /// including its force, container, protection, and controller rules.
    pub(super) fn run_receiving_eject_pilot_with_effects(
        &mut self,
        id: ObjectId,
        ground_creation_list: Option<&str>,
        air_creation_list: Option<&str>,
        damage_dealer: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some(actor) = self.objects.get(&id) else {
            return Ok(());
        };
        let position = actor.get_position();
        let ground = self
            .terrain_ground_height_at(position)
            .unwrap_or(actor.ground_height);
        let list = if actor.height_treats_as_airborne(position.y - ground) {
            air_creation_list
        } else {
            ground_creation_list
        };
        let Some(list) = list else {
            return Ok(());
        };
        self.execute_receiving_eject_pilot_list(id, list, damage_dealer, interpreter, effects)
    }

    /// EjectPilotDie::ejectPilot is also called by HelicopterSlowDeathUpdate.
    pub(super) fn execute_receiving_eject_pilot_list(
        &mut self,
        id: ObjectId,
        list: &str,
        damage_dealer: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        use crate::game_logic::host_usa_pilot::{PILOT_EJECT_AUDIO, PILOT_SOUND_EJECT_AUDIO};

        if !self.has_object_creation_list(list) {
            return Ok(());
        }
        let Some(actor) = self.objects.get(&id) else {
            return Ok(());
        };
        let actor_birth = (actor.visual_object_generation, self.visual_world_epoch);
        let outcome = self.execute_receiving_object_ocl_with_secondary(
            list,
            id,
            damage_dealer,
            interpreter,
            effects,
        );
        if let Some(error) = outcome.error {
            return Err(error);
        }

        let mut ejected = false;
        let mut parachuted = false;
        let mut protected = false;
        for (child, (generation, epoch)) in outcome.created {
            if !self.spawn_identity_matches(child, generation, epoch) {
                continue;
            }
            let child = &self.objects[&child];
            if child.thing().template.contain_module.kind == ContainModuleKind::Parachute {
                continue;
            }
            ejected = true;
            protected |= child.is_eject_invulnerable();
            parachuted |= child
                .contained_by
                .and_then(|id| self.objects.get(&id))
                .is_some_and(|container| {
                    container.thing().template.contain_module.kind == ContainModuleKind::Parachute
                });
        }
        if ejected {
            self.usa_pilot.record_ejection();
        }
        if parachuted {
            self.usa_pilot.record_air_ejection();
        }
        if protected {
            self.usa_pilot.record_invulnerable_grant();
        }

        // EjectPilotDie submits these cues after the OCL, even if a
        // RequiresLivePlayer nugget declined to create an object.
        if self.spawn_identity_matches(id, actor_birth.0, actor_birth.1) {
            let player_index = self.objects[&id]
                .owner_player_id
                .map(|player| player as i32);
            self.queue_resolved_per_unit_sound(
                id,
                PILOT_EJECT_AUDIO,
                false,
                true,
                player_index,
                170,
            );
            self.queue_resolved_per_unit_sound(id, PILOT_SOUND_EJECT_AUDIO, false, true, None, 170);
        }
        Ok(())
    }
}
