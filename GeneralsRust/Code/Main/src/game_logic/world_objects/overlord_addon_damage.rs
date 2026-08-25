//! C++ OverlordContain/HelixContain `onBodyDamageStateChange` on the live host.
//!
//! Spawns the portable payload occupant and calls `setDamageState` (skip BODY_RUBBLE).

use super::super::*;
use crate::game_logic::host_battlemaster::is_portable_structure_template;
use crate::game_logic::host_overlord_addon_damage::{
    installed_portable_addon_kind, overlord_addon_mirrored_damage_state,
    overlord_addon_payload_template, portable_addon_installed,
    should_grant_stealth_to_portable_addon,
};
use crate::game_logic::host_overlord_addons::{is_emperor_template, is_helix_template};

impl GameLogic {
    fn ensure_overlord_payload_template(&mut self, name: &str) {
        if self.templates.contains_key(name) {
            return;
        }
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        self.templates.insert(name.to_string(), template);
    }

    pub(crate) fn ensure_overlord_portable_addon_occupant(
        &mut self,
        host_id: ObjectId,
    ) -> Option<ObjectId> {
        let (kind, is_helix, team, pos, owner, existing, stealthed) = {
            let host = self.objects.get(&host_id)?;
            let emperor = is_emperor_template(&host.template_name);
            let kind = installed_portable_addon_kind(
                host.has_overlord_gattling_addon,
                host.has_overlord_propaganda_addon,
                host.overlord_bunker_slot_capacity(),
                emperor,
            )?;
            let is_helix = host.is_helix_transport || is_helix_template(&host.template_name);
            (
                kind,
                is_helix,
                host.team,
                host.get_position(),
                host.owner_player_id,
                host.overlord_portable_occupant,
                host.is_effectively_stealthed(),
            )
        };
        let template = overlord_addon_payload_template(kind, is_helix);
        if let Some(id) = existing {
            if self.objects.get(&id).is_some_and(|occ| {
                occ.template_name == template && !occ.status.destroyed && occ.is_alive()
            }) {
                self.attach_overlord_portable_occupant(host_id, id);
                self.mirror_overlord_addon_damage_to_occupant(host_id);
                return Some(id);
            }
            self.detach_overlord_portable_occupant(host_id, id);
        }
        self.ensure_overlord_payload_template(template);
        let occupant_id = self.create_object_for_owner_or_team(template, team, owner, pos)?;
        self.attach_overlord_portable_occupant(host_id, occupant_id);
        if stealthed {
            if let Some(occ) = self.objects.get_mut(&occupant_id) {
                if should_grant_stealth_to_portable_addon(
                    true,
                    is_portable_structure_template(&occ.template_name),
                ) {
                    occ.apply_grant_stealth();
                }
            }
        }
        self.mirror_overlord_addon_damage_to_occupant(host_id);
        Some(occupant_id)
    }

    fn attach_overlord_portable_occupant(&mut self, host_id: ObjectId, occupant_id: ObjectId) {
        if let Some(occ) = self.objects.get_mut(&occupant_id) {
            occ.set_contained_by(Some(host_id));
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            host.overlord_portable_occupant = Some(occupant_id);
            if let Some(pos) = host.occupants.iter().position(|&id| id == occupant_id) {
                if pos != 0 {
                    host.occupants.remove(pos);
                    host.occupants.insert(0, occupant_id);
                }
            } else {
                host.occupants.insert(0, occupant_id);
            }
            host.sync_overlord_addon_body_damage();
        }
    }

    fn detach_overlord_portable_occupant(&mut self, host_id: ObjectId, occupant_id: ObjectId) {
        if let Some(host) = self.objects.get_mut(&host_id) {
            host.occupants.retain(|id| *id != occupant_id);
            if host.overlord_portable_occupant == Some(occupant_id) {
                host.overlord_portable_occupant = None;
            }
        }
        if let Some(occ) = self.objects.get_mut(&occupant_id) {
            occ.set_contained_by(None);
            occ.status.destroyed = true;
        }
    }

    pub(crate) fn mirror_overlord_addon_damage_to_occupant(&mut self, host_id: ObjectId) {
        let (maybe_state, occupant_hint, candidates, pos) = {
            let Some(host) = self.objects.get(&host_id) else {
                return;
            };
            if !portable_addon_installed(
                host.has_overlord_gattling_addon,
                host.has_overlord_propaganda_addon,
                host.overlord_bunker_slot_capacity(),
                is_emperor_template(&host.template_name),
            ) {
                return;
            }
            (
                overlord_addon_mirrored_damage_state(host.body_damage_state),
                host.overlord_portable_occupant,
                host.occupants.clone(),
                host.get_position(),
            )
        };
        let Some(state) = maybe_state else {
            return;
        };
        let occupant_id = occupant_hint.or_else(|| {
            candidates.into_iter().find(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|occ| is_portable_structure_template(&occ.template_name))
            })
        });
        let Some(occupant_id) = occupant_id else {
            if let Some(host) = self.objects.get_mut(&host_id) {
                host.overlord_addon_body_damage_state = state;
            }
            return;
        };
        if let Some(occ) = self.objects.get_mut(&occupant_id) {
            if occ.body_damage_state != state {
                occ.apply_overlord_addon_set_damage_state(state);
            }
            occ.set_position(pos);
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            host.overlord_addon_body_damage_state = state;
            host.overlord_portable_occupant = Some(occupant_id);
        }
    }

    pub(crate) fn mirror_overlord_addon_damage_after_combat(&mut self) {
        let hosts: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|obj| {
                portable_addon_installed(
                    obj.has_overlord_gattling_addon,
                    obj.has_overlord_propaganda_addon,
                    obj.overlord_bunker_slot_capacity(),
                    is_emperor_template(&obj.template_name),
                )
            })
            .map(|obj| obj.id)
            .collect();
        for id in hosts {
            self.mirror_overlord_addon_damage_to_occupant(id);
        }
    }

    /// Live occupant id for the hull portable addon, if any.
    /// C++ OverlordContain contain-list front / HelixContain `m_portableStructureID`.
    pub(crate) fn overlord_helix_portable_occupant_id(
        &self,
        host_id: ObjectId,
    ) -> Option<ObjectId> {
        let host = self.objects.get(&host_id)?;
        if let Some(id) = host.overlord_portable_occupant {
            if self.objects.contains_key(&id) {
                return Some(id);
            }
        }
        host.occupants.iter().copied().find(|&id| {
            self.objects
                .get(&id)
                .is_some_and(|occ| is_portable_structure_template(&occ.template_name))
        })
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
            addon.set_team_and_owner(new_team, owner);
        }
        self.attach_overlord_portable_occupant(host_id, addon_id);
        Some(addon_id)
    }
}
