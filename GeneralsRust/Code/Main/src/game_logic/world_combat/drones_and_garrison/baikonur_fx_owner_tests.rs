use super::*;
use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
use glam::Vec3;

#[test]
fn baikonur_neutron_spawn_fx_shakes_only_receiving_view() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "baikonur_neutron_spawn_fx_shakes_only_receiving_view",
        || {
            let mut definition = FXList::new("FX_Nuke".into());
            definition.add_nugget(FXNugget::ViewShake {
                shake_type: CameraShakeType::Normal,
            });
            let list = game_client::fx_list::FXList::from_definition(&definition);
            game_client::fx_list::get_fx_list_store_mut().add_fx_list("FX_Nuke".into(), list);
            game_client::display::view::with_tactical_view(|view| {
                let target = game_client::display::view::Point3::new(-10000., -20000., 7.);
                view.set_position(&target);
                view.reset_camera_shake();
            });
            let core = || {
                game_client::display::view::with_tactical_view_ref(|view| {
                    let point = view.position();
                    (point.x, point.y, point.z, view.camera_shake_intensity())
                })
            };
            let before = core();
            fn world() -> (GameLogic, crate::game_logic::ObjectId) {
                let mut world = GameLogic::new();
                world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
                world.add_player(Player::new(1, Team::USA, "Local", true));
                let mut template = ThingTemplate::new("ReceivingBaikonurTower");
                template.set_health(5000.);
                world
                    .templates
                    .insert("ReceivingBaikonurTower".into(), template);
                let source = world
                    .create_object("ReceivingBaikonurTower", Team::USA, Vec3::ZERO)
                    .unwrap();
                {
                    let mut shroud = world.world_services.shroud().lock().unwrap();
                    shroud.init_shroud_grid_at_origin([-2048., -2048.], 4096., 4096.);
                    shroud.reveal_map_for_player_permanently(1).unwrap();
                }
                world.session_view.target = Vec3::ZERO;
                (world, source)
            }
            let (mut owner, source) = world();
            let (peer, peer_source) = world();
            assert_eq!(source, peer_source);
            assert!(owner.activate_baikonur_detonation(source, Vec3::ZERO));
            assert!(owner.session_view.impulse.intensity > 0.);
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert_eq!(core(), before);
            assert_eq!(
                owner.special_power_strikes.neutron_slow_death_field_count(),
                1
            );
            assert_eq!(
                peer.special_power_strikes.neutron_slow_death_field_count(),
                0
            );
            assert!(
                owner
                    .objects
                    .registered_values()
                    .any(|object| object.pending_death_fx.as_deref()
                        == Some(crate::game_logic::host_baikonur_launch::BAIKONUR_NUKE_FX))
            );
        },
    );
}
