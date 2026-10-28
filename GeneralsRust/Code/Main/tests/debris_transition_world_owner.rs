//! Exercise an authored damage-transition debris OCL through its receiving world.
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};

fn world(model: &str, mass: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(7, Team::USA, "Owner", true));
    world
        .admit_object_ini_sources([(
            "DebrisTransitionOwner.ini".to_owned(),
            "Object DebrisTransitionVictim\nKindOf = VEHICLE SELECTABLE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
             Behavior = TransitionDamageFX TransitionTag\n\
             DamagedOCL1 = Loc: X:0 Y:0 Z:0 OCL:OCL_OwnedDebris\nEnd\nEnd\n\
             Object GenericDebris\nKindOf = PROJECTILE\n\
             Body = ActiveBody DebrisBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
             Draw = W3DDebrisDraw DebrisDrawTag\nEnd\nEnd\n"
                .to_owned(),
        )])
        .unwrap();
    world
        .admit_object_creation_list_sources([format!(
            "ObjectCreationList OCL_OwnedDebris\nCreateDebris\nModelNames = {model}\n\
             Count = 2\nMass = {mass}\nDisposition = SEND_IT_FLYING\n\
             DispositionIntensity = 1\nEnd\nEnd\n"
        )])
        .unwrap();
    let victim = world
        .create_object_for_player("DebrisTransitionVictim", 7, glam::Vec3::ZERO)
        .unwrap();
    (world, victim)
}

fn debris(world: &GameLogic) -> Vec<(String, f32)> {
    world
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "GenericDebris")
        .map(|object| (object.current_visual_model_name(), object.physics_mass))
        .collect()
}

#[test]
fn transition_debris_uses_each_worlds_catalog_and_lod_counter() {
    let (mut first, first_victim) = world("FirstChunk", 5.0);
    let (mut peer, peer_victim) = world("PeerChunk", 9.0);
    assert_eq!(first_victim, peer_victim);
    first.admit_dynamic_debris_skip_mask(1);

    first.apply_host_damage(first_victim, 60.0);
    assert_eq!(debris(&first), vec![("FirstChunk".to_owned(), 5.0)]);
    assert!(debris(&peer).is_empty());

    peer.apply_host_damage(peer_victim, 60.0);
    assert_eq!(debris(&peer), vec![("PeerChunk".to_owned(), 9.0); 2]);
    assert_eq!(debris(&first), vec![("FirstChunk".to_owned(), 5.0)]);

    let first_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 7);
    let peer_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut peer, 7);
    assert!(
        first_frame.unit_render_inputs().iter().any(|draw| {
            draw.template_name == "GenericDebris" && draw.model_key == "FirstChunk"
        })
    );
    assert!(
        peer_frame
            .unit_render_inputs()
            .iter()
            .any(|draw| { draw.template_name == "GenericDebris" && draw.model_key == "PeerChunk" })
    );
}

#[test]
fn debris_generation_save_continuation_is_owned_by_the_restored_world() {
    use generals_main::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

    let builder = SnapshotBuilder::new();
    let (mut source, victim) = world("SavedChunk", 5.0);
    let (mut restored, restored_victim) = world("SavedChunk", 5.0);
    let (mut peer, peer_victim) = world("PeerChunk", 9.0);
    assert_eq!(victim, restored_victim);
    assert_eq!(victim, peer_victim);
    for logic in [&mut source, &mut restored, &mut peer] {
        logic.admit_dynamic_debris_skip_mask(3);
    }

    source.apply_host_damage(victim, 60.0);
    peer.apply_host_damage(peer_victim, 60.0);
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    assert_eq!(snapshot.dynamic_debris_generations, 2);
    let peer_before = builder
        .create_world_snapshot(&peer)
        .unwrap()
        .dynamic_debris_generations;
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();

    let mut invalid: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    invalid.audio_policy.version = 0;
    let restored_before = builder
        .create_world_snapshot(&restored)
        .unwrap()
        .dynamic_debris_generations;
    assert!(
        builder
            .restore_from_snapshot(&invalid, &mut restored)
            .is_err()
    );
    assert_eq!(
        builder
            .create_world_snapshot(&restored)
            .unwrap()
            .dynamic_debris_generations,
        restored_before
    );

    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for logic in [&mut source, &mut restored] {
        let next = logic
            .create_object_for_player("DebrisTransitionVictim", 7, glam::Vec3::X)
            .unwrap();
        logic.apply_host_damage(next, 60.0);
    }
    assert_eq!(debris(&restored), debris(&source));
    assert_eq!(
        builder
            .create_world_snapshot(&restored)
            .unwrap()
            .dynamic_debris_generations,
        4
    );
    assert_eq!(
        builder
            .create_world_snapshot(&peer)
            .unwrap()
            .dynamic_debris_generations,
        peer_before
    );
}

#[test]
fn direct_xfer_preserves_debris_counter_and_following_record() {
    use generals_main::save_load::{Snapshot, WorldSnapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut snapshot = WorldSnapshot::default();
    snapshot.dynamic_debris_generations = 17;
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        snapshot.xfer(&mut writer).unwrap();
        writer.xfer_u32(&mut 0xA3B4_C5D6).unwrap();
    }
    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).unwrap();
        reader.xfer_u32(&mut sentinel).unwrap();
    }
    assert_eq!(restored.dynamic_debris_generations, 17);
    assert_eq!(sentinel, 0xA3B4_C5D6);
}
