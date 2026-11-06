//! C++ OverlordContain/HelixContain `onBodyDamageStateChange` on the live host.
//!
//! Mirrors the host's body damage state onto its actual contained rider.

use super::super::*;
use crate::game_logic::host_overlord_addon_damage::overlord_addon_mirrored_damage_state;

impl GameLogic {
    pub(crate) fn attach_overlord_portable_occupant(
        &mut self,
        host_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let grant_stealth = self.objects.get(&host_id)
            .is_some_and(|host| host.is_helix_transport && host.status.stealthed);
        if let Some(occ) = self.objects.get_mut(&occupant_id) {
            let newly_contained = occ.contained_by != Some(host_id);
            let containment_actor_id = occ.id;
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                &mut self.ai_groups,
                &mut self.pathfinding_system,
                containment_actor_id,
                Some(host_id),
                false,
            );
            let occ = self
                .objects
                .get_mut(&containment_actor_id)
                .expect("containment operation preserves this birth");
            occ.set_experience_sink(Some(host_id));
            if newly_contained && grant_stealth
                && occ.is_kind_of(KindOf::PortableStructure)
                && occ.authors_behavior_module("StealthUpdate")
            {
                occ.apply_grant_stealth();
            }
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            host.overlord_portable_occupant = Some(occupant_id);
            host.overlord_redirection_active = host.is_overlord_style_container();
            if let Some(pos) = host.occupants.iter().position(|&id| id == occupant_id) {
                if pos != 0 {
                    host.occupants.remove(pos);
                    host.occupants.insert(0, occupant_id);
                }
            } else {
                host.occupants.insert(0, occupant_id);
            }
        }
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

        let (maybe_state, pos) = {
            let Some(host) = self.objects.get(&host_id) else {
                return;
            };
            (
                overlord_addon_mirrored_damage_state(host.body_damage_state),
                host.get_position(),
            )
        };
        let Some(state) = maybe_state else {
            return;
        };
        let occupant_id = self.overlord_helix_portable_occupant_id(host_id);
        let Some(occupant_id) = occupant_id else {
            return;
        };
        if let Some(occ) = self.objects.get_mut(&occupant_id) {
            if occ.body_damage_state != state {
                occ.apply_overlord_addon_set_damage_state(state);
            }
        }
        self.set_owned_object_position_with_effects(occupant_id, pos, self.frame, effects);
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

    /// C++ OverlordContain.cpp:227-235 / HelixContain.cpp:217-222 `onCapture`.
    /// `setTeam` the portable rider to the capturer default team; keep attached.
    pub(crate) fn on_capture_overlord_helix_portable_addon(
        &mut self,
        host_id: ObjectId,
        new_team: Team,
    ) -> Option<ObjectId> {
        let addon_id = self.overlord_helix_portable_occupant_id(host_id)?;
        let owner = self
            .objects
            .get(&host_id)
            .and_then(|host| host.owner_player_id);
        if let Some(addon) = self.objects.get_mut(&addon_id) {
            addon.set_team_and_owner_with_partition_owner(
                new_team,
                owner,
                &mut self.ownership_events,
                &mut self.partition_manager,
            );
        }
        self.attach_overlord_portable_occupant(host_id, addon_id);
        Some(addon_id)
    }
}
