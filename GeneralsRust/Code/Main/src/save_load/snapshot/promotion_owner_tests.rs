//! Receiving promotion content and transient UI lifetime across encoded restore.
use super::*;
use crate::game_logic::{GameLogic, Player, Team};
use glam::Vec3;

fn promotion_catalog_world(body_cue: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(2, Team::USA, "ReceivingLocal", true));
    let object_source = format!(
        "Object SavedPromotionUnit\nKindOf = VEHICLE SELECTABLE\nIsTrainable = Yes\nExperienceRequired = 0 50 150 300\nSoundPromotedVeteran = {body_cue}\nSoundPromotedElite = {body_cue}\nSoundPromotedHero = {body_cue}\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "owned-promotion-object.ini".to_string(),
        object_source,
    )])
    .unwrap();
    world.admit_receiving_module_content(content).unwrap();
    let mut definitions = crate::game_logic::promotion_definitions::PromotionDefinitions::from_ordered_sources(
        &[],
        &[("owned-promotion-misc.ini", "MiscAudio\nUnitPromoted = ReceivingMisc\nEnd\n".to_string())],
        &[("owned-promotion-animation.ini", "Animation OwnedLevelGain\nNumberImages = 1\nImage = OwnedLevelGainImage\nAnimationMode = LOOP\nEnd\n".to_string())],
        &|name| name == "OwnedLevelGainImage",
    ).unwrap();
    definitions.rules.animation_name = "OwnedLevelGain".into();
    definitions.rules.display_seconds = 2.0;
    world.admit_promotion_definitions(definitions).unwrap();
    world
}

#[test]
fn encoded_restore_rebinds_body_promotion_cue_from_actual_receiving_catalog() {
    let mut source = promotion_catalog_world("ForeignBodyCue");
    let id = source
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        source.objects[&id].thing().template.sound_promoted[0].as_deref(),
        Some("ForeignBodyCue")
    );
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiving = promotion_catalog_world("ReceivingBodyCue");
    // No templates inserted or copied: restoration must resolve actual private content.
    assert_eq!(
        receiving.templates["SavedPromotionUnit"].sound_promoted[0].as_deref(),
        Some("ReceivingBodyCue")
    );
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert!(receiving.queued_audio_events.is_empty(), "restore is inert");
    assert_eq!(
        receiving.objects[&id].thing().template.sound_promoted[0].as_deref(),
        Some("ReceivingBodyCue")
    );
    receiving.award_experience(id, 60.0);
    let names = receiving
        .queued_audio_events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["ReceivingBodyCue", "ReceivingMisc"]);
    assert_eq!(receiving.session_view.world_animations.births().count(), 1);
    assert!(source.queued_audio_events.is_empty());
}

#[test]
fn legacy_without_camera_record_restore_clears_receiving_transient_animations() {
    let mut source = promotion_catalog_world("SourceBodyCue");
    let _id = source
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::ZERO)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let mut saved = builder.create_world_snapshot(&source).unwrap();
    // Remove only the optional framed CAMC record at its structural boundary.
    // Preserve every following lifecycle record; never search arbitrary bytes.
    let start =
        super::combat_projectile_persist::historical_prefix_len(&saved.lifecycle_tail).unwrap();
    assert_eq!(&saved.lifecycle_tail[start..start + 4], b"CAMC");
    let length = u32::from_le_bytes(
        saved.lifecycle_tail[start + 8..start + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    saved.lifecycle_tail.drain(start..start + 12 + length);
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let mut receiving = promotion_catalog_world("ReceivingBodyCue");
    let old = receiving
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::ZERO)
        .unwrap();
    receiving.award_experience(old, 60.0);
    assert_eq!(receiving.session_view.world_animations.births().count(), 1);
    let mut peer = promotion_catalog_world("PeerBodyCue");
    let peer_id = peer
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::ZERO)
        .unwrap();
    peer.award_experience(peer_id, 60.0);
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving.session_view.world_animations.births().count(),
        0,
        "all successful restores clear nonserialized UI births, even without CAMC"
    );
    assert!(receiving.queued_audio_events.is_empty());
    assert_eq!(peer.session_view.world_animations.births().count(), 1);
}

#[test]
fn direct_restore_preflight_failure_preserves_promotion_birth_audio_and_peer() {
    let mut receiving = promotion_catalog_world("ReceivingBodyCue");
    receiving.frame = 77;
    let id = receiving
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::new(3.0, 0.0, 7.0))
        .unwrap();
    receiving.award_experience(id, 60.0);
    let mut peer = promotion_catalog_world("PeerBodyCue");
    peer.frame = 91;
    let peer_id = peer
        .create_object_for_player("SavedPromotionUnit", 2, Vec3::new(9.0, 0.0, 4.0))
        .unwrap();
    assert_eq!(
        peer_id, id,
        "private owners may reuse raw object identities"
    );
    peer.award_experience(peer_id, 60.0);
    let observe = |world: &GameLogic| {
        let object = &world.objects[&id];
        (
            world.frame,
            world
                .session_view
                .world_animations
                .births()
                .cloned()
                .collect::<Vec<_>>(),
            world
                .queued_audio_events
                .iter()
                .map(|event| {
                    (
                        event.event_type.clone(),
                        event.object_id,
                        event.position,
                        event.priority,
                        event.is_looping,
                        event.stop,
                        event.player_index,
                    )
                })
                .collect::<Vec<_>>(),
            object.experience.current,
            object.experience.level,
            object.health.current,
            object.health.maximum,
        )
    };
    let receiving_before = observe(&receiving);
    let peer_before = observe(&peer);
    assert_eq!(receiving_before.1.len(), 1);
    assert_eq!(receiving_before.1[0].spawn_frame, 77);
    assert_eq!(
        receiving_before
            .2
            .iter()
            .map(|event| event.0.as_str())
            .collect::<Vec<_>>(),
        vec!["ReceivingBodyCue", "ReceivingMisc"]
    );
    let builder = SnapshotBuilder::new();
    for invalid_version in [true, false] {
        let mut saved = builder.create_world_snapshot(&peer).unwrap();
        if !invalid_version {
            // Structurally encodable, but rejected by the receiving geometry preflight.
            saved.shroud.grid = Some(ShroudGridSnapshot {
                width: 1,
                height: 1,
                cell_size: 0.0,
                cells: vec![ShroudCellSnapshot::default()],
            });
        }
        let encoded = bincode_legacy::serialize(&saved).unwrap();
        let mut decoded = decode_bincode_world_snapshot(&encoded).unwrap();
        if invalid_version {
            // Decode a real supported envelope first; exercise the DIRECT API's guard.
            decoded.version = u32::MAX;
        }
        let error = builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap_err();
        if !invalid_version {
            assert!(
                error
                    .to_string()
                    .contains("shroud snapshot cell size is invalid"),
                "{error}"
            );
        }
        assert_eq!(
            observe(&receiving),
            receiving_before,
            "preflight rejection must precede all receiving mutation"
        );
        assert_eq!(
            observe(&peer),
            peer_before,
            "peer remains independently owned"
        );
    }
    // This contract is specifically preflight rejection; direct restore is not
    // claimed to roll back failures after its first mutating replacement step.
}
