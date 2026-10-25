//! Actual Object radius producers allocate through the receiving client owner.
use super::*;
use crate::game_logic::{GameLogic, NativeTickProgress, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;
fn client_position(host: Vec3) -> Vec3 {
    Vec3::new(host.x, host.z, host.y)
}

struct RadiusView<'a>(game_client::core::game_client::NativeDrawableOwner<'a>);
impl NativeViewEffects for RadiusView<'_> {
    fn projected_radius_allocator(&mut self) -> Option<&mut dyn NativeProjectedRadiusAllocator> {
        Some(&mut self.0)
    }
    fn local_player_id(&self) -> u32 {
        0
    }
    fn host_epoch(&self) -> u64 {
        1
    }
    fn sample_slave(&mut self, _: &NativeSlaveSource, _: &str) -> SlaveCameraSample {
        unreachable!("radius creation is not a camera operation")
    }
    fn body_visual_changed(
        &mut self,
        _: &NativeBodyVisualChange,
        _: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        Err("radius probe does not handle Body callbacks".into())
    }
    fn retire_physical_object_drawable(
        &mut self,
        _: &NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Err("radius probe does not retire Drawables".into())
    }
    fn admit_factory_drawable(
        &mut self,
        _: NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        Err("radius probe does not construct Drawables".into())
    }
}
fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "RadiusOwner", true));
    let template = ThingTemplate::new("GLAScudStormReceivingRadiusProbe");
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("GLAScudStormReceivingRadiusProbe", Team::USA, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().owner_player_id = Some(0);
    world.require_native_drawable_owner();
    (world, id)
}

#[test]
fn native_entry_rejects_missing_radius_owner_before_time_or_callbacks() {
    let (mut world, _) = world();
    let before = world.sim_timing_snapshot();
    let mut callback_count = 0;
    let result = world.begin_native_logic_tick(
        1.0 / 30.0,
        None,
        None,
        ScriptPresentationEffects::default(),
        |_| {
            callback_count += 1;
            None
        },
        |_| {},
    );
    assert!(matches!(result, NativeTickProgress::Failed(_)));
    assert_eq!(callback_count, 0);
    let after = world.sim_timing_snapshot();
    assert_eq!(before.frame, after.frame);
    assert_eq!(
        before.accumulated_time_seconds,
        after.accumulated_time_seconds
    );
}

#[test]
fn actual_world_radius_creation_clone_drop_and_clear_are_receiving_client_local() {
    let (mut source, id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(id, peer_id);
    let mut source_client = game_client::core::game_client::NativeDrawableWorld::default();
    let mut peer_client = game_client::core::game_client::NativeDrawableWorld::default();
    let source_pos = Vec3::new(271., 0., 283.);
    let peer_pos = Vec3::new(-291., 0., 307.);
    {
        let mut view = RadiusView(source_client.borrow_native());
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        effects.validate_projected_radius_recipient(true).unwrap();
        assert!(source.create_delivery_radius_decal_with_effects(id, source_pos, 0., &mut effects));
    }
    {
        let mut view = RadiusView(peer_client.borrow_native());
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        effects.validate_projected_radius_recipient(true).unwrap();
        assert!(peer.create_delivery_radius_decal_with_effects(
            peer_id,
            peer_pos,
            0.,
            &mut effects
        ));
    }
    let items = |client: &mut game_client::core::game_client::NativeDrawableWorld| {
        client.borrow_native().collect_projected_decal_items(true)
    };
    assert!(
        items(&mut source_client)
            .iter()
            .any(|item| item.position == client_position(source_pos))
    );
    assert!(
        items(&mut source_client)
            .iter()
            .all(|item| item.position != client_position(peer_pos))
    );
    assert!(
        items(&mut peer_client)
            .iter()
            .any(|item| item.position == client_position(peer_pos))
    );
    drop(source.host_object(id).unwrap().clone());
    assert!(
        items(&mut source_client)
            .iter()
            .any(|item| item.position == client_position(source_pos))
    );
    let frozen = items(&mut source_client);
    let moved = Vec3::new(311., 7., 317.);
    source
        .host_object_mut(id)
        .unwrap()
        .radius_decal_update
        .as_mut()
        .unwrap()
        .delivery_decal
        .set_position(moved);
    assert!(
        frozen
            .iter()
            .any(|item| item.position == client_position(source_pos))
    );
    assert!(
        items(&mut source_client)
            .iter()
            .any(|item| item.position == client_position(moved)),
        "same actual handle receives the immediate original XYZ pose before render"
    );
    source
        .host_object_mut(id)
        .unwrap()
        .radius_decal_update
        .as_mut()
        .unwrap()
        .kill_radius_decal();
    assert!(items(&mut source_client).is_empty());
    assert!(
        items(&mut peer_client)
            .iter()
            .any(|item| item.position == client_position(peer_pos))
    );
    // Inert construction/drop never selects or clears another receiving client.
    drop(game_client::core::game_client::NativeDrawableWorld::default());
    assert!(
        items(&mut peer_client)
            .iter()
            .any(|item| item.position == client_position(peer_pos))
    );
    // The existing native candidate/reset teardown releases this owner's live
    // ring even when it has no Object-to-Drawable association.
    peer_client
        .borrow_native()
        .try_clear_for_map_attempt(None)
        .unwrap();
    assert!(items(&mut peer_client).is_empty());
    assert!(items(&mut source_client).is_empty());
}

#[test]
fn encoded_world_radius_reconstruction_allocates_only_in_receiving_client() {
    let (mut source, id) = world();
    let mut source_client = game_client::core::game_client::NativeDrawableWorld::default();
    let source_pos = Vec3::new(401., 0., 419.);
    {
        let mut view = RadiusView(source_client.borrow_native());
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        assert!(source.create_delivery_radius_decal_with_effects(id, source_pos, 0., &mut effects));
    }
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
    // Generic snapshot replacement operates on a detached candidate; the
    // source's live native actor/client resources stay active throughout.
    let mut restored = GameLogic::new();
    // Admit the same receiving player/team prototype before strict roster
    // restoration, without creating an Object or native resource.
    restored.add_player(Player::new(0, Team::USA, "RadiusOwner", true));
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.host_object(id).unwrap().id, id);
    restored.require_native_drawable_owner();
    assert!(
        !restored
            .host_object(id)
            .unwrap()
            .radius_decal_update
            .as_ref()
            .unwrap()
            .delivery_decal
            .has_projected_shadow()
    );
    let mut restored_client = game_client::core::game_client::NativeDrawableWorld::default();
    assert!(
        restored_client
            .borrow_native()
            .collect_projected_decal_items(true)
            .is_empty()
    );
    let restored_pos = Vec3::new(-431., 0., 443.);
    {
        let mut view = RadiusView(restored_client.borrow_native());
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        assert!(restored.create_delivery_radius_decal_with_effects(
            id,
            restored_pos,
            0.,
            &mut effects
        ));
    }
    let source_items = source_client
        .borrow_native()
        .collect_projected_decal_items(true);
    assert!(
        source_items
            .iter()
            .any(|item| item.position == client_position(source_pos))
    );
    assert!(
        source_items
            .iter()
            .all(|item| item.position != client_position(restored_pos))
    );
    let restored_items = restored_client
        .borrow_native()
        .collect_projected_decal_items(true);
    assert!(
        restored_items
            .iter()
            .any(|item| item.position == client_position(restored_pos))
    );
    assert!(
        restored_items
            .iter()
            .all(|item| item.position != client_position(source_pos))
    );
}
