//! C++ OverlordContain/HelixContain `onBodyDamageStateChange` on the live host.
//!
//! Mirrors the host's body damage state onto its actual contained rider.

use super::super::*;
use crate::game_logic::host_overlord_addon_damage::overlord_addon_mirrored_damage_state;

impl GameLogic {
    #[cfg(test)]
    pub(crate) fn attach_overlord_portable_occupant(
        &mut self,
        host_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        assert!(self.attach_overlord_portable_with_effects(host_id, occupant_id, &mut Default::default()));
    }

    pub(crate) fn attach_overlord_portable_with_effects(
        &mut self,
        host_id: ObjectId,
        occupant_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(host) = self.objects.get(&host_id) else { return false };
        let Some(occupant) = self.objects.get(&occupant_id) else { return false };
        if occupant.contained_by == Some(host_id) && host.overlord_portable_occupant == Some(occupant_id) {
            return true;
        }
        // HelixContain::addToContain bypasses OpenContain entirely for its
        // separate portable slot: only the ID and contained-by pointer change.
        if host.is_helix_transport {
            self.objects.get_mut(&occupant_id).unwrap().contained_by = Some(host_id);
            self.objects.get_mut(&host_id).unwrap().overlord_portable_occupant = Some(occupant_id);
            return true;
        }
        let experience_sink = host.is_overlord_style_container()
            && host.get_template().contain_module.experience_sink_for_rider;
        let grant_stealth = host.status.stealthed
            && occupant.contained_by != Some(host_id)
            && occupant.is_kind_of(KindOf::PortableStructure);
        let (host_birth, occupant_birth) = (host.visual_object_generation, occupant.visual_object_generation);
        let epoch = self.visual_world_epoch;
        let host = self.objects.get_mut(&host_id).unwrap();
        let old_index = host.occupants.iter().position(|&id| id == occupant_id);
        if old_index != Some(0) {
            if let Some(index) = old_index {
                host.occupants.remove(index);
            }
            host.occupants.insert(0, occupant_id);
        }
        self.set_owned_passenger_held(occupant_id, true, effects);
        if !self.spawn_identity_matches(host_id, host_birth, epoch) {
            return false;
        }
        if !self.spawn_identity_matches(occupant_id, occupant_birth, epoch) {
            // The callback can retire the new rider before onContainedBy
            // installs its back-reference. Remove the staged roster entry.
            if old_index.is_none() {
                self.objects.get_mut(&host_id).unwrap().occupants.retain(|&id| id != occupant_id);
            }
            return false;
        }
        let host = self.objects.get_mut(&host_id).unwrap();
        host.overlord_portable_occupant = Some(occupant_id);
        let bit = crate::game_logic::host_enum_table_residual::loaded_model_bit();
        host.model_condition_bits |= 1u128 << bit;
        host.overlord_redirection_active = host.is_overlord_style_container();
        if experience_sink {
            self.objects.get_mut(&occupant_id).unwrap().set_experience_sink(Some(host_id));
        }
        if grant_stealth {
            self.receive_stealth_grant(occupant_id, true, 0);
        }
        self.objects.get_mut(&occupant_id).unwrap().finish_containment_reference_with_partition_owner(
            Some(host_id), false, &mut self.partition_manager,
        );
        true
    }

    pub(crate) fn mirror_overlord_addon_damage_to_occupant(&mut self, host_id: ObjectId) {
        self.mirror_overlord_addon_damage_to_occupant_with_effects(
            host_id,
            &mut Default::default(),
        );
    }

    pub(crate) fn mirror_overlord_addon_damage_to_occupant_with_effects(
        &mut self,
        host_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {

        let (maybe_state, pos, helix_orientation) = {
            let Some(host) = self.objects.get(&host_id) else {
                return;
            };
            (
                overlord_addon_mirrored_damage_state(host.body_damage_state),
                host.get_position(),
                host.is_helix_transport.then(|| host.get_orientation()),
            )
        };
        let occupant_id = self.overlord_helix_portable_occupant_id(host_id);
        let Some(occupant_id) = occupant_id else {
            return;
        };
        if let (Some(occ), Some(state)) = (self.objects.get_mut(&occupant_id), maybe_state) {
            if occ.body_damage_state != state {
                occ.apply_overlord_addon_set_damage_state(state);
            }
        }
        self.set_owned_object_position_with_effects(occupant_id, pos, self.frame, effects);
        if let Some(orientation) = helix_orientation {
            if let Some((occupant, partition)) = self.host_object_and_partition_mut(occupant_id) {
                occupant.set_orientation_with_partition_owner(orientation, partition);
            }
        }
    }

    pub(crate) fn mirror_overlord_addon_damage_after_combat_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let hosts: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|obj| obj.has_portable_overlord_addon())
            .map(|obj| obj.id)
            .collect();
        for id in hosts {
            self.mirror_overlord_addon_damage_to_occupant_with_effects(id, effects);
        }
    }

    /// Live occupant id for the hull portable addon, if any.
    /// C++ OverlordContain contain-list front / HelixContain `m_portableStructureID`.
    pub(crate) fn overlord_helix_portable_occupant_id(
        &self,
        host_id: ObjectId,
    ) -> Option<ObjectId> {
        let host = self.objects.get(&host_id)?;
        let id = host.overlord_portable_occupant?;
        self.objects.get(&id)
            .filter(|occ| occ.contained_by == Some(host_id))
            .map(|_| id)
    }

    /// OverlordContain.cpp:227–235 / HelixContain.cpp:217–222 use the
    /// receiving controller's default Team. A Helix has a separate portable
    /// slot, whereas an Overlord picks the first physical rider.
    pub(in crate::game_logic) fn capture_overlord_helix_payload_with_effects(
        &mut self,
        host_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(host) = self.objects.get(&host_id) else {
            return Ok(());
        };
        let payload = if host.is_helix_transport {
            host.overlord_portable_occupant
        } else {
            host.occupants.first().copied()
        };
        let Some(payload) = payload.filter(|payload| {
            self.objects
                .get(payload)
                .is_some_and(|rider| rider.contained_by == Some(host_id))
        }) else {
            return Ok(());
        };
        let Some(owner) = host.owner_player_id else {
            return Ok(());
        };
        let destination = self.host_default_team_id(owner).ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "captured portable payload requires its controller's default Team".into(),
            )
        })?;
        // Neither the host Object nor a Team guard is borrowed into the
        // child transfer. Object::setTeam runs the child's capture callbacks
        // synchronously and keeps the physical attachment.
        if !self.set_host_object_team_with_effects(payload, destination, effects)? {
            return Err(gamelogic::GameLogicError::Configuration(
                "captured portable payload could not enter its controller's default Team".into(),
            ));
        }
        Ok(())
    }
}
