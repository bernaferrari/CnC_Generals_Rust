//! C++ FXList.cpp794-805: primary visibility precedes synchronous nuggets.
//! Exercise Main's actual object publication and the real GameClient bridge.

use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug)]
struct Observation {
    nugget: u8,
    primary: HostFxObjectPose,
    secondary: Option<HostFxObjectPose>,
}

struct ObjectNugget {
    nugget: u8,
    observed: Sender<Observation>,
}

impl game_client::fx_list::FXNugget for ObjectNugget {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("the object dispatch must retain its pose and secondary identity");
    }

    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        // A nugget may inspect or admit definitions; no catalog lock may span
        // the callback. The event capture itself needs no shared mutable Vec.
        drop(game_client::fx_list::get_fx_list_store_mut());
        self.observed
            .send(Observation {
                nugget: self.nugget,
                primary: *primary,
                secondary: secondary.copied(),
            })
            .unwrap();
    }
}

fn install_fx() -> Receiver<Observation> {
    let (observed, events) = mpsc::channel();
    let mut fx = game_client::fx_list::FXList::new();
    for nugget in [1, 2] {
        fx.add_fx_nugget(Box::new(ObjectNugget {
            nugget,
            observed: observed.clone(),
        }));
    }
    game_client::fx_list::get_fx_list_store_mut().add_fx_list("FX_OwnerVisibility".into(), fx);
    game_client::fx_list::register_fx_list_manager_bridge();
    events
}

fn world(local: u32, x: f32) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    // Admit this owner explicitly; FX needs no retail map/content bootstrap.
    logic.install_as_active_stores();
    logic.clear_all_players();
    logic.add_player(Player::new(local, Team::USA, "Local", true));
    logic.add_player(Player::new(8, Team::China, "Enemy", false));
    let template = ThingTemplate::new("FxOwnerUnit");
    logic.templates.insert(template.name.clone(), template);
    let primary = logic
        .create_object("FxOwnerUnit", Team::China, Vec3::new(x, 4.0, 12.0))
        .unwrap();
    let secondary = logic
        .create_object("FxOwnerUnit", Team::China, Vec3::new(x + 10.0, 6.0, 25.0))
        .unwrap();
    logic
        .host_object_mut(primary)
        .unwrap()
        .set_orientation(0.75);
    assert_eq!(logic.local_player_id(), Some(local));
    (logic, primary, secondary)
}

fn status(logic: &GameLogic, player: u32, object: ObjectId, value: ObjectShroudStatus) {
    logic
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(player, object.0, value);
}

fn dispatch(logic: &mut GameLogic, primary: ObjectId, secondary: ObjectId) {
    assert!(logic.dispatch_fx_list_at_host_object("FX_OwnerVisibility", primary, Some(secondary)));
}

fn assert_visible(events: &Receiver<Observation>, primary: ObjectId, secondary: ObjectId, x: f32) {
    let observations: Vec<_> = events.try_iter().collect();
    assert_eq!(observations.len(), 2, "both actual client nuggets ran");
    for (event, nugget) in observations.iter().zip([1, 2]) {
        assert_eq!(event.nugget, nugget, "C++ authored nugget order");
        assert_eq!(event.primary.id, primary.0);
        assert_eq!(event.primary.position, Coord3D::new(x, 12.0, 4.0));
        assert!(!event.primary.is_shrouded);
        assert_eq!(event.primary.player_index, 8);
        let source = event.secondary.unwrap();
        assert_eq!(source.id, secondary.0);
        assert_eq!(source.position, Coord3D::new(x + 10.0, 25.0, 6.0));
        assert!(
            source.is_shrouded,
            "secondary visibility never suppresses primary FX"
        );
    }
}

#[test]
fn object_fx_uses_own_visibility_local_player_and_pose_with_reused_ids() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "object_fx_uses_own_visibility_local_player_and_pose_with_reused_ids",
        || {
            let events = install_fx();
            let (mut left, primary, secondary) = world(0, 10.0);
            let (mut right, right_primary, right_secondary) = world(7, 100.0);
            assert_eq!((primary, secondary), (right_primary, right_secondary));
            status(&left, 0, primary, ObjectShroudStatus::Fogged);
            status(&left, 0, secondary, ObjectShroudStatus::Shrouded);
            status(&right, 7, primary, ObjectShroudStatus::Clear);
            status(&right, 7, secondary, ObjectShroudStatus::Shrouded);
            // Opposite status for another local slot catches a global-player
            // read even if it happens to select the correct world's shroud.
            status(&right, 0, primary, ObjectShroudStatus::Shrouded);
            dispatch(&mut left, primary, secondary);
            assert_eq!(
                events.try_iter().count(),
                0,
                "left fogged primary is suppressed"
            );
            dispatch(&mut right, primary, secondary);
            assert_visible(&events, primary, secondary, 100.0);
            status(&left, 0, primary, ObjectShroudStatus::PartialClear);
            status(&right, 7, primary, ObjectShroudStatus::Shrouded);
            dispatch(&mut left, primary, secondary);
            assert_visible(&events, primary, secondary, 10.0);
            dispatch(&mut right, primary, secondary);
            assert_eq!(
                events.try_iter().count(),
                0,
                "right primary is independently suppressed"
            );
        },
    );
}

#[test]
fn object_fx_keeps_driving_visibility_after_foreign_construct_reset_and_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "object_fx_keeps_driving_visibility_after_foreign_construct_reset_and_restore",
        || {
            let events = install_fx();
            let (mut left, primary, secondary) = world(0, 20.0);
            status(&left, 0, primary, ObjectShroudStatus::PartialClear);
            status(&left, 0, secondary, ObjectShroudStatus::Shrouded);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&left).unwrap();
            let active = gamelogic::system::engine_stores::active_services();
            let candidate = GameLogic::new();
            assert!(
                std::sync::Arc::ptr_eq(
                    &active,
                    &gamelogic::system::engine_stores::active_services()
                ),
                "constructing a candidate never publishes it"
            );
            dispatch(&mut left, primary, secondary);
            assert_visible(&events, primary, secondary, 20.0);
            let (mut right, _, _) = world(7, 200.0);
            status(&right, 7, primary, ObjectShroudStatus::Shrouded);
            right.reset();
            dispatch(&mut left, primary, secondary);
            assert_visible(&events, primary, secondary, 20.0);
            status(&left, 0, primary, ObjectShroudStatus::Fogged);
            dispatch(&mut left, primary, secondary);
            assert_eq!(events.try_iter().count(), 0);
            builder.restore_from_snapshot(&saved, &mut left).unwrap();
            // Object visibility is derived transient state, not a new save
            // field. Observe this owner's newly admitted post-load statuses.
            status(&left, 0, primary, ObjectShroudStatus::PartialClear);
            status(&left, 0, secondary, ObjectShroudStatus::Shrouded);
            dispatch(&mut left, primary, secondary);
            assert_visible(&events, primary, secondary, 20.0);
            drop(candidate);
            drop(right);
            dispatch(&mut left, primary, secondary);
            assert_visible(&events, primary, secondary, 20.0);
        },
    );
}

#[test]
fn object_fx_missing_local_player_fails_closed_and_unknown_status_keeps_host_policy() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "object_fx_missing_local_player_fails_closed_and_unknown_status_keeps_host_policy",
        || {
            let events = install_fx();
            let (mut logic, primary, secondary) = world(0, 30.0);
            logic
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .clear_host_object_visibility(0);
            status(&logic, 0, secondary, ObjectShroudStatus::Shrouded);
            assert!(
                logic
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(0, primary.0)
                    .is_none()
            );
            dispatch(&mut logic, primary, secondary);
            assert_visible(&events, primary, secondary, 30.0);
            for player in logic.get_players_mut().values_mut() {
                player.is_local = false;
            }
            assert_eq!(logic.local_player_id(), None);
            dispatch(&mut logic, primary, secondary);
            assert_eq!(events.try_iter().count(), 0, "no local viewer fails closed");
        },
    );
}

#[test]
fn object_fx_keeps_owned_pose_when_foreign_core_object_has_same_id() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "object_fx_keeps_owned_pose_when_foreign_core_object_has_same_id",
        || {
            let events = install_fx();
            let (mut logic, primary, secondary) = world(0, 40.0);
            status(&logic, 0, primary, ObjectShroudStatus::Clear);
            status(&logic, 0, secondary, ObjectShroudStatus::Shrouded);
            // This independent Core fixture constructs AI-backed players;
            // admit and retire it under its own explicit store scope. Keep
            // that foreign scope active during the actual Main dispatch.
            let core_stores = gamelogic::system::engine_stores::new_for_world();
            gamelogic::system::engine_stores::with_active_stores(&core_stores, || {
                let mut core = gamelogic::system::game_logic::GameLogic::new();
                let foreign = std::sync::Arc::new(std::sync::RwLock::new(
                    gamelogic::object::Object::new_raw(
                        std::sync::Arc::new(gamelogic::common::DefaultThingTemplate::new(
                            "ForeignCoreFxOwner".into(),
                        )),
                        primary.0,
                        gamelogic::common::ObjectStatusMaskType::none(),
                        None,
                    ),
                ));
                core.register_object(std::sync::Arc::clone(&foreign))
                    .unwrap();
                {
                    let foreign_guard = foreign.write().unwrap();
                    assert_ne!(*foreign_guard.get_position(), Coord3D::new(40.0, 12.0, 4.0));
                    // C++ gets this owner's actual object. A same-ID foreign
                    // Core entry must never replace it or be reacquired here.
                    dispatch(&mut logic, primary, secondary);
                    assert_visible(&events, primary, secondary, 40.0);
                }
                core.destroy_object(primary.0);
                core.process_destroy_list().unwrap();
                assert!(
                    gamelogic::object::registry::OBJECT_REGISTRY
                        .get_object(primary.0)
                        .is_none()
                );
                assert!(
                    !logic.dispatch_fx_list_at_host_object(
                        "FX_OwnerVisibility",
                        ObjectId(u32::MAX - 1),
                        Some(secondary)
                    ),
                    "missing owned primary cannot reuse a foreign/cache object"
                );
                assert_eq!(events.try_iter().count(), 0);
            });
        },
    );
}

fn install_session_shake_definition(name: &str) {
    use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
    let mut definition = FXList::new(name.into());
    definition.add_nugget(FXNugget::ViewShake {
        shake_type: CameraShakeType::Normal,
    });
    game_client::fx_list::get_fx_list_store_mut().add_fx_list(
        name.into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
}

fn reveal_fx_positions(world: &GameLogic, player: u32) {
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(256.0, 256.0);
    shroud.reveal_map_for_player_permanently(player).unwrap();
}

#[test]
fn authored_fx_observes_canonical_session_target_and_not_foreign_tactical_view() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_fx_observes_canonical_session_target_and_not_foreign_tactical_view",
        || {
            install_session_shake_definition("FX_SessionViewOwner");
            let rules = game_engine::common::global_data::read().clone();
            assert!(rules.shake_normal_intensity > 0.0 && rules.max_shake_range > 0.0);
            let (mut left, id, _) = world(0, 10.0);
            let (mut peer, peer_id, _) = world(7, 10.0);
            assert_eq!(id, peer_id);
            status(&left, 0, id, ObjectShroudStatus::Clear);
            status(&peer, 7, id, ObjectShroudStatus::Clear);
            left.session_view.target = Vec3::new(10.0, 9000.0, 12.0);
            peer.session_view.target = Vec3::new(10.0 + rules.max_shake_range * 2.0, 0.0, 12.0);
            game_client::display::view::with_tactical_view(|view| {
                view.set_position(&game_client::display::view::Point3::new(
                    -100000.0, -100000.0, 0.0,
                ));
                view.reset_camera_shake();
            });
            assert!(left.dispatch_fx_list_at_host_object("FX_SessionViewOwner", id, None));
            assert_eq!(
                left.session_view.impulse.intensity,
                if rules.shake_normal_intensity > rules.max_shake_intensity {
                    3.0
                } else {
                    rules.shake_normal_intensity
                }
            );
            assert_eq!(
                left.session_view.impulse.offset,
                Vec3::ZERO,
                "enqueue must not advance view"
            );
            let saved = left.session_view.capture();
            assert!(peer.dispatch_fx_list_at_host_object("FX_SessionViewOwner", id, None));
            assert_eq!(peer.session_view.impulse.intensity, 0.0);
            let _inert_candidate = GameLogic::new();
            assert_eq!(left.session_view.capture(), saved);
            assert_eq!(
                game_client::display::view::with_tactical_view_ref(
                    |view| view.camera_shake_intensity()
                ),
                0.0
            );
            // A target write immediately changes the SAME owner read by positional FX.
            reveal_fx_positions(&left, 0);
            left.session_view.target = Vec3::new(rules.max_shake_range * 2.0 + 10.0, 0.0, 12.0);
            let intensity = left.session_view.impulse.intensity;
            assert!(left.dispatch_fx_list_at_host_position(
                "FX_SessionViewOwner",
                Vec3::new(10.0, 4.0, 12.0)
            ));
            assert_eq!(left.session_view.impulse.intensity, intensity);
            left.session_view.target = Vec3::new(10.0, 0.0, 12.0);
            left.session_view.impulse.intensity = 0.0;
            assert!(left.dispatch_fx_list_at_host_position(
                "FX_SessionViewOwner",
                Vec3::new(10.0, 4.0, 12.0)
            ));
            assert!(left.session_view.impulse.intensity > 0.0);
        },
    );
}

#[test]
fn authored_nested_bone_fx_borrows_same_session_impulse_and_world_visibility() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_nested_bone_fx_borrows_same_session_impulse_and_world_visibility",
        || {
            use game_engine::common::ini::ini_fx_list::{FXList, FXNugget};
            install_session_shake_definition("FX_SessionViewChild");
            let mut definition = FXList::new("FX_SessionViewBone".into());
            definition.add_nugget(FXNugget::FXListAtBonePos {
                fx_name: "FX_SessionViewChild".into(),
                bone_name: "Muzzle".into(),
                orient_to_bone: false,
            });
            let outer = game_client::fx_list::FXList::from_definition(&definition);
            let (mut logic, id, _) = world(0, 10.0);
            reveal_fx_positions(&logic, 0);
            logic.session_view.target = Vec3::new(10.0, 0.0, 12.0);
            let primary = HostFxObjectPose {
                id: id.0,
                position: Coord3D::new(10.0, 12.0, 4.0),
                transform: Matrix3D::from_translation(glam::Vec3::new(10.0, 12.0, 4.0)),
                player_index: 0,
                bounding_circle_radius: 1.0,
                is_shrouded: false,
            };
            let mut visits = Vec::new();
            logic.with_host_fx_view(|services| {
                // Actual factory bone access requires a rendered Drawable. Supply
                // only its observed matrices here; all nested authored FX, shroud
                // queries, catalog lookups and impulse writes are the real route.
                let mut bones = |object, name: &str, start, limit| {
                    assert_eq!(object, id.0);
                    assert_eq!(name, "Muzzle");
                    assert_eq!(limit, 40);
                    visits.push(start);
                    vec![Matrix3D::IDENTITY]
                };
                let mut receiving = game_client::fx_list::FxViewServices {
                    shake: &mut *services.shake,
                    find_list: &mut *services.find_list,
                    bone_transforms: &mut bones,
                    position_visible: &mut *services.position_visible,
                };
                outer.do_fx_obj_host_with_view(&primary, None, &mut receiving);
            });
            assert_eq!(visits, vec![0, 1]);
            assert!(logic.session_view.impulse.intensity > 0.0);
            assert_eq!(logic.session_view.impulse.offset, Vec3::ZERO);
            logic.session_view.advance_impulse();
            assert!(logic.session_view.impulse.offset.length_squared() > 0.0);
        },
    );
}
