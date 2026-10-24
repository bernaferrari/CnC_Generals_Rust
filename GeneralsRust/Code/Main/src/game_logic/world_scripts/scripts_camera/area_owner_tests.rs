use super::*;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;
fn triangle(offset: i32) -> PolygonTrigger {
    PolygonTrigger::new(
        1,
        AsciiString::from("OwnedArea"),
        vec![
            ICoord3D::new(offset, offset, 0),
            ICoord3D::new(offset + 10, offset, 0),
            ICoord3D::new(offset, offset + 10, 0),
        ],
    )
}
#[test]
fn receiving_area_queries_and_map_install_never_publish_or_consume_foreign_tracker() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_area_queries_and_map_install_never_publish_or_consume_foreign_tracker",
        || {
            let owner = GameLogic::new();
            let peer = GameLogic::new();
            let tracker = gamelogic::scripting::engine::get_area_tracker();
            tracker
                .register_area(gamelogic::scripting::events::TriggerArea::new_rectangular(
                    "OwnedArea".into(),
                    [75.0, 75.0, 0.0],
                    70.0,
                    70.0,
                    80.0,
                    80.0,
                ))
                .unwrap();
            let before = tracker.get_area_aabb("OwnedArea");
            let stage = gamelogic::runtime_world_transaction::HostRuntimeWorldStage::begin(
                &owner.world_services,
            );
            assert_eq!(
                tracker.get_area_aabb("OwnedArea"),
                before,
                "Main stage never takes standalone area state"
            );
            {
                let mut terrain = owner.world_services.terrain().write().unwrap();
                crate::game_logic::script_loader::install_runtime_polygon_triggers(
                    &mut terrain,
                    &[triangle(0)],
                );
            }
            {
                let mut terrain = peer.world_services.terrain().write().unwrap();
                crate::game_logic::script_loader::install_runtime_polygon_triggers(
                    &mut terrain,
                    &[triangle(100)],
                );
            }
            assert_eq!(
                tracker.get_area_aabb("OwnedArea"),
                before,
                "actual map admission leaves standalone tracker intact"
            );
            assert!(
                owner.host_script_point_in_trigger(glam::Vec3::new(2.0, 0.0, 2.0), "OwnedArea")
            );
            assert!(
                !owner.host_script_point_in_trigger(glam::Vec3::new(8.0, 0.0, 8.0), "OwnedArea"),
                "polygon, not its bounding rectangle"
            );
            assert!(
                !peer.host_script_point_in_trigger(glam::Vec3::new(2.0, 0.0, 2.0), "OwnedArea")
            );
            assert!(
                peer.host_script_point_in_trigger(glam::Vec3::new(102.0, 0.0, 102.0), "OwnedArea")
            );
            let center = owner.host_script_area_center("OwnedArea").unwrap();
            assert_eq!((center.x, center.z), (5.0, 5.0));
            let center = peer.host_script_area_center("OwnedArea").unwrap();
            assert_eq!((center.x, center.z), (105.0, 105.0));
            assert!(owner.host_script_area_center("ForeignOnly").is_none());
            owner
                .world_services
                .terrain()
                .write()
                .unwrap()
                .replace_receiving_trigger_areas([]);
            assert!(
                !owner.host_script_point_in_trigger(glam::Vec3::new(2.0, 0.0, 2.0), "OwnedArea")
            );
            assert!(peer.host_script_area_center("OwnedArea").is_some());
            let candidate = stage.finish_and_restore_live(&owner.world_services);
            drop(candidate);
            assert_eq!(tracker.get_area_aabb("OwnedArea"), before);
        },
    );
}
