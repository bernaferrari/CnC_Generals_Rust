use super::*;
use crate::game_logic::object::entity_lifecycle_tags::TAG_FIRING_TRACKER;
use crate::game_logic::{ObjectId, Team, ThingTemplate};
use gamelogic::world::entities::EntityModuleState;

#[test]
fn prior_positional_tracker_payload_decodes_without_restoring_unsaved_idle_reload() {
    let old = FiringTrackerV1 {
        consecutive_shots: 5,
        consecutive_target: Some(ObjectId(9)),
        continuous_fire_level: 1,
        coast_until_frame: 200,
        force_reload_frame: 150,
    };
    let bytes = encode_payload(&old).unwrap();
    let mut owner = Object::new(
        ThingTemplate::new("OldTrackerOwner"),
        ObjectId(1),
        Team::USA,
    );
    decode(&bytes).unwrap().apply(&mut owner);
    assert_eq!(owner.consecutive_shots_at_target, 5);
    assert_eq!(owner.consecutive_shot_target, Some(ObjectId(9)));
    assert_eq!(owner.continuous_fire_coast_until_frame, 200);
    assert_eq!(owner.continuous_fire_level, 1);
    assert_eq!(owner.frame_to_force_reload, 0);
}

#[test]
fn malformed_tracker_preflight_does_not_apply_earlier_modules() {
    use crate::game_logic::object::entity_lifecycle_inventory::PathFollowResidual;
    use crate::game_logic::object::entity_lifecycle_tags::TAG_PATH_EXTRA_DISTANCE;
    let mut owner = Object::new(
        ThingTemplate::new("InvalidTrackerOwner"),
        ObjectId(1),
        Team::USA,
    );
    owner.path_extra_distance = 7.0;
    owner.consecutive_shots_at_target = 2;
    for bad in [
        FiringTrackerPersist {
            consecutive_shots: u32::MAX,
            consecutive_target: None,
            continuous_fire_level: 0,
            coast_until_frame: 50,
        },
        FiringTrackerPersist {
            consecutive_shots: 1,
            consecutive_target: Some(ObjectId(0)),
            continuous_fire_level: 0,
            coast_until_frame: 50,
        },
        FiringTrackerPersist {
            consecutive_shots: 1,
            consecutive_target: None,
            continuous_fire_level: 3,
            coast_until_frame: 50,
        },
    ] {
        let mut bytes = MAGIC.to_vec();
        bytes.push(VERSION);
        bytes.extend_from_slice(&encode_payload(&bad).unwrap());
        let modules = [
            EntityModuleState {
                tag: TAG_PATH_EXTRA_DISTANCE.into(),
                payload: encode_payload(&PathFollowResidual {
                    path_extra_distance: 99.0,
                    is_exact_path: true,
                    is_attack_path: false,
                })
                .unwrap(),
            },
            EntityModuleState {
                tag: TAG_FIRING_TRACKER.into(),
                payload: bytes,
            },
        ];
        assert!(
            crate::game_logic::object::entity_lifecycle_apply::apply_module_states(
                &mut owner, &modules
            )
            .is_err()
        );
        assert_eq!(owner.path_extra_distance, 7.0);
        assert_eq!(owner.consecutive_shots_at_target, 2);
    }
}
