//! Actual receiving particle beam update and authored ground-hit ViewShake.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;
fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world
}

#[test]
#[cfg(feature = "game_client")]
fn native_particle_ground_hit_uses_receiving_camera_not_core_view() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_particle_ground_hit_uses_receiving_camera_not_core_view",
        || {
            use crate::game_logic::special_power_strikes::PARTICLE_GROUND_HIT_FX;
            use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
            let mut definition = FXList::new(PARTICLE_GROUND_HIT_FX.into());
            definition.add_nugget(FXNugget::ViewShake {
                shake_type: CameraShakeType::Normal,
            });
            game_client::fx_list::get_fx_list_store_mut().add_fx_list(
                definition.name.to_string(),
                game_client::fx_list::FXList::from_definition(&definition),
            );
            let rules = game_engine::common::global_data::read().clone();
            assert!(rules.max_shake_range > 0. && rules.shake_normal_intensity > 0.);
            let mut owner = world();
            let mut peer = world();
            let position = Vec3::new(10., 0., 12.);
            owner.session_view.target = position;
            peer.session_view.target = position + Vec3::X * rules.max_shake_range * 2.;
            for world in [&mut owner, &mut peer] {
                {
                    let mut shroud = world.world_services.shroud().lock().unwrap();
                    shroud.init_shroud_grid_at_origin([-2048., -2048.], 4096., 4096.);
                    shroud.reveal_map_for_player_permanently(1).unwrap();
                }
                world.special_power_strikes.spawn_beam_field(
                    ObjectId(9991),
                    Team::USA,
                    position,
                    0,
                    0,
                );
            }
            game_client::display::view::with_tactical_view(|view| {
                view.set_position(&game_client::display::view::Point3::new(
                    -100000., -100000., 0.,
                ));
                view.reset_camera_shake();
            });
            peer.update_particle_beam_fields();
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            owner.update_particle_beam_fields();
            assert!(
                owner.session_view.impulse.intensity > 0.,
                "OLD free scorch dispatch only shakes foreign Core View"
            );
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert_eq!(
                owner.special_power_strikes.beam_fields()[0].scorch_marks_made,
                1
            );
            game_client::display::view::with_tactical_view_ref(|view| {
                assert_eq!(view.camera_shake_intensity(), 0.)
            });
        },
    );
}
