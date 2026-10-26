//! Actual installed Object -> receiving terrain -> FX kernel -> trigger tail.
use super::*;
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
use game_engine::common::audio::{AudioEventRts, AudioSubmissionLocality};
use game_engine::common::ini::ini_fx_list::{FXList, FXNugget};
use std::sync::{Arc, Mutex};
struct Audio {
    trigger: Arc<Mutex<gamelogic::scripting::HostTriggerWorld>>,
    names: Vec<String>,
}
impl crate::game_logic::NativeAudioEffects for Audio {
    fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
        assert!(
            self.trigger.lock().unwrap().capture().is_empty(),
            "tree FX observes original pre-trigger phase"
        );
        assert_eq!(
            event.get_position().map(|pos| (pos.x, pos.y, pos.z)),
            Some((3.0, 0.0, 0.0))
        );
        self.names.push(event.get_event_name().to_owned());
    }
}
fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Local", true));
    let mut template = ThingTemplate::new("TreeContactVehicle");
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, ObjectId(41), Team::USA);
    object.owner_player_id = Some(1);
    object.selection_radius = 8.0;
    object.crusher_level = 2;
    object.movement.velocity = glam::Vec3::new(4.0, 0.0, 0.0);
    object.attach_host_trigger_world(&world.host_trigger_world);
    world.objects.insert(object.id, object);
    world.frame = 37;
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_current_frame(37);
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
    let mut definition = FXList::new("OwnedTreeTopple".into());
    definition.add_nugget(FXNugget::Sound {
        name: "TreeToppleVoice".into(),
    });
    let mut catalog = game_client::fx_list::FXListStore::new();
    catalog.add_fx_list(
        "OwnedTreeTopple".into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
    world.admit_fx_catalog(catalog);
    world
}
fn terrain() -> game_client::terrain::terrain_visual::TerrainVisualImpl {
    let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    visual
        .tree_buffer_mut()
        .set_bounds(game_client::terrain::TreeRegion2D::new(
            glam::Vec2::ZERO,
            glam::Vec2::splat(100.0),
        ));
    let mut data = game_client::terrain::TreeModuleData::default();
    data.model_name = "OwnedTree".into();
    data.do_topple = true;
    data.topple_fx = Some("OwnedTreeTopple".into());
    visual
        .tree_buffer_mut()
        .add_tree(
            701,
            glam::Vec3::new(3.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            data,
            game_client::terrain::TreeSphere {
                center: glam::Vec3::ZERO,
                radius: 5.0,
            },
        )
        .unwrap();
    visual
}
#[test]
fn installed_motion_topple_fx_precedes_trigger_update_and_is_consumed_once() {
    let mut world = world();
    let mut receiving = terrain();
    let mut peer = terrain();
    let mut audio = Audio {
        trigger: Arc::clone(&world.host_trigger_world),
        names: Vec::new(),
    };
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        terrain: Some(&mut receiving),
        audio: Some(&mut audio),
        ..Default::default()
    };
    assert!(
        world
            .drive_owned_contact_with_effects(
                ObjectId(41),
                MotionContact::Start(0.0),
                Object::step_motion_contact_with_effects,
                &mut effects
            )
            .is_some()
    );
    drop(effects);
    assert_eq!(audio.names, vec!["TreeToppleVoice"]);
    assert!(receiving.tree_buffer_mut().pending_fx_events().is_empty());
    assert!(peer.tree_buffer_mut().pending_fx_events().is_empty());
    assert_eq!(
        peer.tree_buffer_mut().trees()[0].topple_state,
        game_client::terrain::W3DToppleState::Upright
    );
    let flags = world.host_trigger_world.lock().unwrap().capture();
    assert_eq!(flags.len(), 1);
    assert_eq!((flags[0].i_x, flags[0].i_y), (4, 0));
    drop(peer);
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        terrain: Some(&mut receiving),
        audio: Some(&mut audio),
        ..Default::default()
    };
    assert!(
        world
            .drive_owned_contact_with_effects(
                ObjectId(41),
                MotionContact::Start(0.0),
                Object::step_motion_contact_with_effects,
                &mut effects
            )
            .is_some()
    );
    drop(effects);
    assert_eq!(
        audio.names,
        vec!["TreeToppleVoice"],
        "same already toppling tree does not replay FX on another movement callback"
    );
}

#[test]
fn owned_position_callback_releases_actor_before_fx_then_resumes_trigger() {
    let mut world = world();
    let mut receiving = terrain();
    let mut audio = Audio {
        trigger: Arc::clone(&world.host_trigger_world),
        names: Vec::new(),
    };
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        terrain: Some(&mut receiving),
        audio: Some(&mut audio),
        ..Default::default()
    };
    let reaction = {
        let (object, partition) = world.host_object_and_partition_mut(ObjectId(41)).unwrap();
        object.prepare_position_tree_reaction(
            glam::Vec3::new(4.0, 0.0, 0.0),
            37,
            &mut effects.reborrow_for_object(partition),
        )
    };
    assert!(reaction.has_tree_fx());
    assert!(
        world
            .host_trigger_world
            .lock()
            .unwrap()
            .capture()
            .is_empty()
    );
    assert!(world.complete_owned_position_reaction(ObjectId(41), reaction, &mut effects));
    drop(effects);
    assert_eq!(audio.names, vec!["TreeToppleVoice"]);
    let flags = world.host_trigger_world.lock().unwrap().capture();
    assert_eq!((flags[0].i_x, flags[0].i_y), (4, 0));
}
