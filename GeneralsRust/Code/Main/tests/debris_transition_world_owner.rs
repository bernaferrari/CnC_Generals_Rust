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
