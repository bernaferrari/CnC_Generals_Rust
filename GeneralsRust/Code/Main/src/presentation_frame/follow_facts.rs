//! Freeze exact receiving follow identity and airborne predicate once per frame.
use super::*;
pub(super) fn freeze(logic: &crate::game_logic::GameLogic) -> Option<follow::FollowTargetFacts> {
    let id = logic.camera_follow_object_id()?;
    let object = logic.objects.get(&id)?;
    let position = object.get_position();
    let airborne_orientation = if object.selected_locomotor_is_airborne() {
        let owner = logic.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        let mut water_height = 0.;
        let surface =
            if terrain.is_underwater(position.x, position.z, Some(&mut water_height), None) {
                water_height
            } else {
                terrain.get_ground_height(position.x, position.z, None)
            };
        (position.y > surface).then_some(object.get_orientation())
    } else {
        None
    };
    Some(follow::FollowTargetFacts {
        id,
        position,
        airborne_orientation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn follow_facts_resolve_only_the_receiving_exact_object_identity() {
        use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
        let mut a = GameLogic::new();
        let mut peer = GameLogic::new();
        a.add_player(Player::new(0, Team::USA, "Owner", true));
        peer.add_player(Player::new(0, Team::USA, "Peer", true));
        a.templates
            .insert("FollowUnit".into(), ThingTemplate::new("FollowUnit"));
        peer.templates
            .insert("FollowUnit".into(), ThingTemplate::new("FollowUnit"));
        let id = a
            .create_object_for_player("FollowUnit", 0, Vec3::new(80., 37., 90.))
            .unwrap();
        let peer_id = peer
            .create_object_for_player("FollowUnit", 0, Vec3::new(800., 370., 900.))
            .unwrap();
        assert_eq!(id, peer_id);
        a.session_view.set_camera_follow(Some(id));
        peer.session_view.set_camera_follow(Some(peer_id));
        assert_eq!(freeze(&a).unwrap().position, Vec3::new(80., 37., 90.));
        assert_eq!(freeze(&peer).unwrap().position, Vec3::new(800., 370., 900.));
        a.session_view
            .set_camera_follow(Some(crate::game_logic::ObjectId(u32::MAX)));
        assert!(freeze(&a).is_none());
        assert_eq!(freeze(&peer).unwrap().id, peer_id);
    }
}

#[cfg(test)]
#[path = "follow_airborne_tests.rs"]
mod airborne_owner_tests;

#[cfg(test)]
#[path = "follow_edge_tests.rs"]
mod edge_owner_tests;
