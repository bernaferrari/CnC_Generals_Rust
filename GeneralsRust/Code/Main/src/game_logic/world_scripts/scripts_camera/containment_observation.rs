//! CPP contain counts belong to the module or its exact communal owner.
use super::*;
use generals_sim::scripting::world::ScriptContainmentStatus;
impl GameLogic {
    /// CPP OverlordContain157: only the active outer front's real interface
    /// redirects. This never searches another world or fabricates a bunker.
    pub(crate) fn redirected_containment_owner(&self, id: ObjectId) -> ObjectId {
        self.host_object(id)
            .map_or(id, |outer| self.redirected_containment_object(outer).id)
    }

    pub(crate) fn redirected_containment_object<'a>(&'a self, outer: &'a Object) -> &'a Object {
        if !outer.is_overlord_style_container() || !outer.overlord_redirection_active {
            return outer;
        }
        let Some(child) = outer.occupants.first().copied() else {
            return outer;
        };
        if child == outer.id {
            return outer;
        }
        self.host_object(child)
            .filter(|object| {
                object.contained_by == Some(outer.id)
                    && object.thing().template.contain_module.kind != ContainModuleKind::None
            })
            .unwrap_or(outer)
    }

    pub(crate) fn contained_units_for_interface(&self, id: ObjectId) -> Vec<ObjectId> {
        self.host_object(self.redirected_containment_owner(id))
            .map(|object| object.contained_units())
            .unwrap_or_default()
    }

    pub(super) fn owned_script_containment(&self, id: ObjectId) -> Option<ScriptContainmentStatus> {
        let object = self.host_object(id)?;
        let metadata = &object.thing().template.contain_module;
        if object.is_overlord_style_container() {
            let child = self.redirected_containment_owner(id);
            if child != id {
                return self.owned_script_containment(child);
            }
            return Some(ScriptContainmentStatus {
                count: object.occupants.len() as u32,
                max: crate::game_logic::host_overlord_addons::OVERLORD_CONTAIN_SLOTS as u32,
            });
        }
        let (count, max) = match metadata.kind {
            ContainModuleKind::Tunnel => {
                let player = object
                    .owner_player_id
                    .filter(|id| self.players.contains_key(id))?;
                (
                    self.tunnel_network.contain_count(player),
                    usize::try_from(self.tunnel_network.capacity()).unwrap_or(0),
                )
            }
            ContainModuleKind::Cave => {
                let index = self.cave_system.index_of(id)?;
                (
                    self.cave_system.contain_count(index),
                    usize::try_from(self.cave_system.capacity()).unwrap_or(0),
                )
            }
            ContainModuleKind::Garrison
            | ContainModuleKind::Heal
            | ContainModuleKind::Parachute => {
                (object.contained_units().len(), metadata.slots.unwrap_or(0))
            }
            ContainModuleKind::Transport
            | ContainModuleKind::RiderChange
            | ContainModuleKind::RailedTransport
            | ContainModuleKind::InternetHack => {
                (object.contained_units().len(), object.max_transport)
            }
            ContainModuleKind::None => return None,
        };
        // CPP NamedHasFreeContainerSlots assigns the signed max to UnsignedInt.
        Some(ScriptContainmentStatus {
            count: count as u32,
            max: max as u32,
        })
    }
}
