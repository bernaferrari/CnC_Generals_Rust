//! CPP contain counts belong to the module or its exact communal owner.
use super::*;
use gamelogic::scripting::engine::ScriptContainmentStatus;
impl GameLogic {
    /// CPP OverlordContain157: only the active outer front's real interface
    /// redirects. This never searches another world or fabricates a bunker.
    pub(crate) fn redirected_containment_owner(&self, id: ObjectId) -> ObjectId {
        let Some(outer) = self.host_object(id) else {
            return id;
        };
        if !outer.is_overlord_style_container() || !outer.overlord_redirection_active {
            return id;
        }
        let Some(child) = outer.occupants.first().copied() else {
            return id;
        };
        if child == id {
            return id;
        }
        if self.host_object(child).is_some_and(|object| {
            object.contained_by == Some(id)
                && object.thing().template.contain_module.kind != ContainModuleKind::None
        }) {
            child
        } else {
            id
        }
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
                    crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY,
                )
            }
            ContainModuleKind::Cave => {
                let index = self.cave_system.index_of(id)?;
                (
                    self.cave_system.contain_count(index),
                    crate::game_logic::host_cave_system::MAX_CAVE_CAPACITY,
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
