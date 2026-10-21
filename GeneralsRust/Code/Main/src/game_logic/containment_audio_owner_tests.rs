//! Actual receiving object callbacks and Common AR_Play admission, without TLS.
use super::*;
use crate::game_logic::ScriptPresentationEffects;
use crate::game_logic::containment_audio_test_support as audio;

pub(crate) fn source(prefix: &str, sounds: &str) -> String {
    format!(
        r#"
Object AudioOwnerContainer
 KindOf = VEHICLE
 SoundEnter = {prefix}Enter
 SoundExit = {prefix}Exit
 Body = ActiveBody BodyTag
  MaxHealth = 200
  InitialHealth = 200
 End
 Behavior = TransportContain ContainTag
  ContainMax = 5
  Slots = 5
  AllowInsideKindOf = INFANTRY
  EnterSound = {sounds}Load
  ExitSound = {sounds}Unload
 End
End
Object AudioOwnerPassenger
 KindOf = INFANTRY
 TransportSlotCount = 1
 SoundFallingFromPlane = {prefix}Falling
 Locomotor = SET_NORMAL BasicHumanLocomotor
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface AiTag
 End
End
"#
    )
}
pub(crate) fn world(prefix: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    audio::admit(&mut world, &source(prefix, prefix));
    world.add_player(Player::new(0, Team::USA, "Audio owner", true));
    let c = world
        .create_object_for_player("AudioOwnerContainer", 0, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let p = world
        .create_object_for_player("AudioOwnerPassenger", 0, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    (world, c, p)
}
fn enter(
    world: &mut GameLogic,
    container: ObjectId,
    passenger: ObjectId,
    output: &mut game_engine::common::audio::AudioManager,
) {
    assert!(world.unit_command_order_enter(passenger, container));
    world.update_support_states_with_effects(
        &[passenger],
        1.0 / 30.0,
        &mut ScriptPresentationEffects {
            audio: Some(output),
            ..Default::default()
        },
    );
    assert_eq!(
        world.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
    assert!(
        world
            .host_object(container)
            .unwrap()
            .contained_units()
            .contains(&passenger)
    );
}
fn remove(
    world: &mut GameLogic,
    container: ObjectId,
    passenger: ObjectId,
    output: &mut game_engine::common::audio::AudioManager,
) {
    assert!(world.unit_command_remove_occupant(container, passenger));
    assert!(world.unit_command_exit_drop_with_effects(
        passenger,
        Vec3::new(50.0, 0.0, 50.0),
        &mut ScriptPresentationEffects {
            audio: Some(output),
            ..Default::default()
        }
    ));
    assert_eq!(world.host_object(passenger).unwrap().contained_by, None);
    assert!(
        !world
            .host_object(container)
            .unwrap()
            .contained_units()
            .contains(&passenger)
    );
}

#[test]
fn native_containment_audio_is_immediate_ordered_and_instance_owned() {
    let (mut a, ac, ap) = world("A");
    let (mut b, bc, bp) = world("B");
    assert_eq!((ac, ap), (bc, bp));
    let mut ao = audio::manager(&["AEnter", "AExit", "AFalling", "ALoad", "AUnload"]);
    let mut bo = audio::manager(&["BEnter", "BExit", "BFalling", "BLoad", "BUnload"]);
    // CPP ctor lastLoad=0: template callback occurs, module cooldown suppresses frame0.
    enter(&mut a, ac, ap, &mut ao);
    audio::assert_events(&ao, &[("AEnter", ac)]);
    a.frame = 7;
    b.frame = 7;
    enter(&mut b, bc, bp, &mut bo);
    remove(&mut a, ac, ap, &mut ao);
    audio::assert_events(
        &ao,
        &[
            ("AEnter", ac),
            ("AUnload", ac),
            ("AExit", ac),
            ("AFalling", ap),
        ],
    );
    enter(&mut a, ac, ap, &mut ao);
    remove(&mut b, bc, bp, &mut bo);
    remove(&mut a, ac, ap, &mut ao);
    enter(&mut a, ac, ap, &mut ao);
    audio::assert_events(
        &ao,
        &[
            ("AEnter", ac),
            ("AUnload", ac),
            ("AExit", ac),
            ("AFalling", ap),
            ("AEnter", ac),
            ("ALoad", ac),
            ("AExit", ac),
            ("AFalling", ap),
            ("AEnter", ac),
        ],
    );
    audio::assert_events(
        &bo,
        &[
            ("BEnter", bc),
            ("BLoad", bc),
            ("BUnload", bc),
            ("BExit", bc),
            ("BFalling", bp),
        ],
    );
    assert_eq!(a.host_object(ac).unwrap().last_load_sound_frame, 7);
    assert_eq!(a.host_object(ac).unwrap().last_unload_sound_frame, 7);
    // A never borrows/changes B's real recipient, even with identical Object IDs.
    a.reset();
    audio::assert_events(
        &bo,
        &[
            ("BEnter", bc),
            ("BLoad", bc),
            ("BUnload", bc),
            ("BExit", bc),
            ("BFalling", bp),
        ],
    );
}

#[test]
fn native_containment_silent_slots_stamp_cooldowns_without_invented_cues() {
    let mut silent = GameLogic::new();
    audio::admit(
        &mut silent,
        r#"
Object AudioOwnerContainer
 KindOf = VEHICLE
 SoundEnter = NoSound
 SoundExit = NONE
 Body = ActiveBody BodyTag
  MaxHealth = 200
  InitialHealth = 200
 End
 Behavior = TransportContain ContainTag
  ContainMax = 5
  Slots = 5
  AllowInsideKindOf = INFANTRY
  EnterSound = NoSound
  ExitSound = NONE
 End
End
Object AudioOwnerPassenger
 KindOf = INFANTRY
 TransportSlotCount = 1
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#,
    );
    silent.add_player(Player::new(0, Team::USA, "Silent", true));
    let c = silent
        .create_object_for_player("AudioOwnerContainer", 0, Vec3::ZERO)
        .unwrap();
    let p = silent
        .create_object_for_player("AudioOwnerPassenger", 0, Vec3::ZERO)
        .unwrap();
    let mut output = audio::manager(&[]);
    silent.frame = 12;
    silent.play_container_enter_sound(
        c,
        &mut ScriptPresentationEffects {
            audio: Some(&mut output),
            ..Default::default()
        },
    );
    silent.play_container_removing_template_sounds(
        c,
        p,
        &mut ScriptPresentationEffects {
            audio: Some(&mut output),
            ..Default::default()
        },
    );
    assert_eq!(silent.host_object(c).unwrap().last_load_sound_frame, 12);
    assert_eq!(silent.host_object(c).unwrap().last_unload_sound_frame, 12);
    audio::assert_events(&output, &[]);
}

#[test]
fn native_containment_encoded_restore_rebinds_audio_to_receiving_template() {
    let (mut a, c, p) = world("Save");
    a.frame = 7;
    let mut before = audio::manager(&["SaveEnter", "SaveLoad"]);
    enter(&mut a, c, p, &mut before);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&a).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world("Save");
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    restored.frame = 8;
    let mut after = audio::manager(&["SaveExit", "SaveUnload", "SaveFalling"]);
    remove(&mut restored, c, p, &mut after);
    audio::assert_events(
        &after,
        &[("SaveUnload", c), ("SaveExit", c), ("SaveFalling", p)],
    );
    audio::assert_events(&before, &[("SaveEnter", c), ("SaveLoad", c)]);
}

#[test]
fn native_module_static_nosound_retains_current_cue_and_none_is_registered_name() {
    let mut world = GameLogic::new();
    let source = source("Static", "Static")
        .replace(
            "EnterSound = StaticLoad",
            "EnterSound = StaticLoad\n  EnterSound = NoSound",
        )
        .replace(
            "ExitSound = StaticUnload",
            "ExitSound = NONE\n  ExitSound = NoSound",
        );
    audio::admit(&mut world, &source);
    world.add_player(Player::new(0, Team::USA, "Static parser", true));
    let c = world
        .create_object_for_player("AudioOwnerContainer", 0, Vec3::ZERO)
        .unwrap();
    let p = world
        .create_object_for_player("AudioOwnerPassenger", 0, Vec3::ZERO)
        .unwrap();
    world.frame = 9;
    let mut output = audio::manager(&[
        "StaticEnter",
        "StaticLoad",
        "StaticExit",
        "StaticFalling",
        "NONE",
    ]);
    world.play_container_enter_sound(
        c,
        &mut ScriptPresentationEffects {
            audio: Some(&mut output),
            ..Default::default()
        },
    );
    world.play_container_removing_template_sounds(
        c,
        p,
        &mut ScriptPresentationEffects {
            audio: Some(&mut output),
            ..Default::default()
        },
    );
    audio::assert_events(
        &output,
        &[
            ("StaticEnter", c),
            ("StaticLoad", c),
            ("NONE", c),
            ("StaticExit", c),
            ("StaticFalling", p),
        ],
    );
}
