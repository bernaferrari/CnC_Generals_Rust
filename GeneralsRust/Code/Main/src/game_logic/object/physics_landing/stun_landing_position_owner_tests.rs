//! Actual receiving owner at the saved-position continuation after Body.
//! The fixture admits the continuation directly; it does not claim to execute
//! an authored Body callback that relocates its source.
use super::*;
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use game_engine::common::{
    audio::{AudioEventRts, AudioSubmissionLocality},
    ini::ini_fx_list::{FXList, FXNugget},
};
#[test]
fn saved_stun_landing_position_delivers_tree_fx_before_resuming_ground_tail() {
    struct Audio(Vec<String>);
    impl crate::game_logic::NativeAudioEffects for Audio {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            self.0.push(event.get_event_name().to_owned());
        }
    }
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(100.0, 100.0);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .reveal_map_for_player_permanently(1)
        .unwrap();
    let id = ObjectId(771);
    let mut template = ThingTemplate::new("StunLandingSource");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let mut object = Object::new(template, id, Team::USA);
    object.set_position(glam::Vec3::new(20.0, 0.0, 20.0));
    object.owner_player_id = Some(1);
    object.selection_radius = 4.0;
    object.crusher_level = 2;
    object.shock_stun_frames = 1;
    world.objects.insert(id, object);
    let mut definition = FXList::new("StunLandingTreeFX".into());
    definition.add_nugget(FXNugget::Sound {
        name: "StunLandingTreeSound".into(),
    });
    let mut catalog = game_client::fx_list::FXListStore::new();
    catalog.add_fx_list(
        "StunLandingTreeFX".into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
    world.admit_fx_catalog(catalog);
    let mut terrain = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    terrain
        .tree_buffer_mut()
        .set_bounds(game_client::terrain::TreeRegion2D::new(
            glam::Vec2::ZERO,
            glam::Vec2::splat(100.0),
        ));
    terrain
        .tree_buffer_mut()
        .add_tree(
            702,
            glam::Vec3::new(2.0, 2.0, 0.0),
            1.0,
            0.0,
            1.0,
            game_client::terrain::TreeModuleData {
                model_name: "StunLandingTree".into(),
                do_topple: true,
                topple_fx: Some("StunLandingTreeFX".into()),
                ..Default::default()
            },
            game_client::terrain::TreeSphere {
                center: glam::Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();
    let landing = StunLanding {
        tail: StunTail {
            already_ground: true,
            was_airborne: false,
        },
        pos: glam::Vec3::new(2.0, 0.0, 2.0),
        ground_y: 0.0,
        old_y: 1.0,
        impact_vy: 0.0,
        was_air: false,
        bounce: 0.0,
    };
    let mut audio = Audio(Vec::new());
    assert!(
        world
            .drive_owned_contact_with_effects(
                id,
                StunContact::AfterBounce(landing),
                Object::step_stun_contact_with_effects,
                &mut crate::game_logic::ScriptPresentationEffects {
                    terrain: Some(&mut terrain),
                    audio: Some(&mut audio),
                    ..Default::default()
                }
            )
            .is_some()
    );
    assert_eq!(audio.0, ["StunLandingTreeSound"]);
    assert!(terrain.tree_buffer_mut().pending_fx_events().is_empty());
    let object = world.objects.get(&id).unwrap();
    assert_eq!(
        object.get_position(),
        glam::Vec3::new(2.0, 0.0, 2.0),
        "original copied translation survives callback phase"
    );
    assert!(object.shock_grounded_once);
}
