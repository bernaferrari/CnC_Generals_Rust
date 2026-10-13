//! Immutable allegiance-change observations owned by the receiving match.
//! Canonical transfer and callbacks stay synchronous; presentation only observes.

use super::{ObjectId, Team};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOwnerEvent {
    pub object: ObjectId,
    pub team: Team,
    /// Exact controlling player when the transfer came through a player-aware
    /// host path. `None` preserves legacy team-only transfer semantics.
    pub owner_player_id: Option<u32>,
}

#[derive(Debug, Default)]
pub struct OwnershipEvents {
    pending: Vec<HostOwnerEvent>,
    completed: Vec<HostOwnerEvent>,
}
impl OwnershipEvents {
    pub fn record(&mut self, object: ObjectId, team: Team, owner_player_id: Option<u32>) {
        self.pending.push(HostOwnerEvent {
            object,
            team,
            owner_player_id,
        });
    }
    pub(crate) fn complete_boundary(&mut self) {
        self.completed.append(&mut self.pending);
    }
    pub(crate) fn take_completed(&mut self) -> Vec<HostOwnerEvent> {
        std::mem::take(&mut self.completed)
    }
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
    }
}

#[cfg(test)]
mod owned_receipt_tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Player, ThingTemplate};
    use crate::presentation_frame::{PresentationEvent, PresentationFrame};
    use glam::Vec3;

    fn world() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Original", true));
        world.add_player(Player::new(1, Team::USA, "SameFaction", false));
        world.add_player(Player::new(2, Team::GLA, "Captured", false));
        let mut template = ThingTemplate::new("OwnershipReceiptInfantry");
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        world.templates.insert(template.name.clone(), template);
        let object = world
            .create_object_for_player("OwnershipReceiptInfantry", 0, Vec3::ZERO)
            .unwrap();
        (world, object)
    }
    #[test]
    fn actual_same_faction_and_enemy_transfers_keep_receiver_fifo_and_saved_allegiance() {
        let (mut owner, object) = world();
        let (mut peer, peer_object) = world();
        assert_eq!(object, peer_object);
        assert!(owner.transfer_object_to_player(object, 1));
        assert_eq!(owner.host_object(object).unwrap().team, Team::USA);
        assert_eq!(owner.host_object(object).unwrap().owner_player_id, Some(1));
        owner.ownership_events.complete_boundary();
        assert!(peer.transfer_object_to_player(peer_object, 2));
        peer.ownership_events.complete_boundary();
        assert_eq!(peer.ownership_events.take_completed().len(), 1);
        peer.reset();
        assert!(owner.transfer_object_to_player(object, 2));
        assert_eq!(owner.host_object(object).unwrap().team, Team::GLA);
        assert_eq!(owner.host_object(object).unwrap().owner_player_id, Some(2));
        owner.ownership_events.complete_boundary();
        assert!(owner.transfer_object_to_player(object, 2));
        owner.ownership_events.complete_boundary();
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&owner).unwrap();
        let (mut peer, peer_object) = world();
        assert!(peer.transfer_object_to_player(peer_object, 1));
        builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
        assert!(peer.ownership_events.take_completed().is_empty());
        assert_eq!(peer.host_object(object).unwrap().owner_player_id, Some(2));
        assert_eq!(peer.host_object(object).unwrap().team, Team::GLA);
        let frame = PresentationFrame::build_from_logic(&mut owner, 0);
        let changes = frame
            .events
            .iter()
            .filter_map(|event| match event {
                PresentationEvent::OwnerChanged { id, team } => Some((*id, *team)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(changes, vec![(object, Team::USA), (object, Team::GLA)]);
        let next = PresentationFrame::build_from_logic(&mut owner, 0);
        assert!(
            !next
                .events
                .iter()
                .any(|event| matches!(event, PresentationEvent::OwnerChanged { .. }))
        );
        assert!(peer.transfer_object_to_player(object, 0));
        assert_eq!(peer.host_object(object).unwrap().owner_player_id, Some(0));
        assert_eq!(owner.host_object(object).unwrap().owner_player_id, Some(2));
    }
    #[test]
    fn kill_pilot_body_neutralization_admits_only_the_receiving_owner() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::object::DamageHitContext;
        let (mut owner, _) = world();
        let (mut peer, _) = world();
        let mut template = ThingTemplate::new("OwnershipReceiptTank");
        template.add_kind_of(KindOf::Vehicle).set_health(200.0);
        owner
            .templates
            .insert(template.name.clone(), template.clone());
        peer.templates.insert(template.name.clone(), template);
        let tank = owner
            .create_object_for_player("OwnershipReceiptTank", 0, Vec3::ZERO)
            .unwrap();
        let peer_tank = peer
            .create_object_for_player("OwnershipReceiptTank", 0, Vec3::ZERO)
            .unwrap();
        assert_eq!(tank, peer_tank);
        let result = owner
            .apply_owned_damage(
                tank,
                1.0,
                None,
                DamageType::KillPilot,
                HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
            .unwrap();
        assert!(!result.destroyed);
        let vehicle = owner.host_object(tank).unwrap();
        assert!(vehicle.is_unmanned());
        assert_eq!(vehicle.health.current, 200.0);
        assert_eq!(vehicle.team, Team::Neutral);
        assert_eq!(vehicle.owner_player_id, None);
        assert_eq!(peer.host_object(peer_tank).unwrap().team, Team::USA);
        peer.ownership_events.complete_boundary();
        assert!(peer.ownership_events.take_completed().is_empty());
        owner.ownership_events.complete_boundary();
        assert_eq!(
            owner.ownership_events.take_completed(),
            vec![HostOwnerEvent {
                object: tank,
                team: Team::Neutral,
                owner_player_id: None
            }]
        );
        assert!(owner.ownership_events.take_completed().is_empty());
    }
}
