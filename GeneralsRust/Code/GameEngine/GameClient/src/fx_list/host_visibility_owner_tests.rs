//! FXList.cpp794-805: consume the primary's frozen owner observation once.
use super::*;
use gamelogic::helpers::HostFxObjectPose;
use std::sync::mpsc;

struct ObjectNugget(mpsc::Sender<HostFxObjectPose>);

impl FXNugget for ObjectNugget {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("real bridge must retain the object nugget route");
    }

    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, _: Option<&HostFxObjectPose>) {
        self.0.send(*primary).unwrap();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn actual_bridge_consumes_host_visibility_without_selecting_ambient_world() {
    const MARKER: &str = "GENERALS_HOST_FX_VISIBILITY_OWNER_CHILD";
    const TEST: &str = "fx_list::host_visibility_owner_tests::actual_bridge_consumes_host_visibility_without_selecting_ambient_world";
    if std::env::var_os(MARKER).is_none() {
        super::borrowed_owner_tests::run_child(TEST, MARKER);
        return;
    }
    const ID: u32 = 0x00F8_0B12;
    const FX: &str = "FX_FrozenHostVisibility";
    assert!(
        gamelogic::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    let (captured, observations) = mpsc::channel();
    let mut fx = FXList::new();
    fx.add_fx_nugget(Box::new(ObjectNugget(captured)));
    get_fx_list_store_mut().add_fx_list(FX.into(), fx);
    let fx_id = NameKeyGenerator::name_to_key(FX) as FXListId;
    let mut pose = HostFxObjectPose {
        id: ID,
        position: Coord3D::new(11.0, 12.0, 13.0),
        transform: Matrix3D::IDENTITY,
        player_index: 7,
        bounding_circle_radius: 20.0,
        is_shrouded: true,
    };
    gamelogic::player::player_list()
        .write()
        .unwrap()
        .set_local_player_index(0);
    let shroud = gamelogic::system::shroud_manager::get_shroud_manager();
    shroud.lock().unwrap().set_host_object_shroud_status(
        0,
        ID,
        gamelogic::common::ObjectShroudStatus::Clear,
    );
    let mut foreign_pose = pose;
    foreign_pose.position = Coord3D::new(900.0, 901.0, 902.0);
    foreign_pose.is_shrouded = false;
    gamelogic::helpers::set_host_fx_object_pose(foreign_pose);
    FXListManagerBridge.do_fx_for_host_objects(fx_id, &pose, None);
    assert_eq!(
        observations.try_iter().count(),
        0,
        "foreign clear state must not leak hidden owner FX"
    );

    pose.is_shrouded = false;
    foreign_pose.is_shrouded = true;
    gamelogic::helpers::set_host_fx_object_pose(foreign_pose);
    let players = gamelogic::player::player_list();
    let mut foreign_players = players.write().unwrap();
    foreign_players.set_local_player_index(-1);
    let mut foreign_shroud = shroud.lock().unwrap();
    foreign_shroud.set_host_object_shroud_status(
        0,
        ID,
        gamelogic::common::ObjectShroudStatus::Shrouded,
    );
    // Held foreign guards make an ambient acquisition observable as a bounded
    // failure, rather than letting coincidentally equal statuses mask it.
    FXListManagerBridge.do_fx_for_host_objects(fx_id, &pose, None);
    let events: Vec<_> = observations.try_iter().collect();
    assert_eq!(
        events.len(),
        1,
        "foreign invalid viewer must not suppress visible owner FX"
    );
    assert_eq!(events[0].position, pose.position);
    assert_eq!(events[0].player_index, 7);
    assert!(!events[0].is_shrouded);
}
