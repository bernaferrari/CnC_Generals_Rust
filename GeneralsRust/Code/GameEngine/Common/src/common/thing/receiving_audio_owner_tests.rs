//! Dynamic ThingTemplate audio slots retain original NoSound clearing semantics.
use super::*;
use crate::common::name_key_generator::NameKeyNamespace;
use crate::common::thing::thing_template::ThingTemplateAudioType;

fn owner(event: &str) -> ReceivingModuleContent {
    let mut content =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    content.load_ini_text(&format!(
        "Object DefaultThingTemplate\nSoundAmbient = {event}\nSoundAmbientDamaged = DamageEvent\nEnd\nObject AudioOwner\nEnd\nObject QuietOwner\nSoundAmbient = nOsOuNd\nEnd\n"
    ), INILoadType::Overwrite).unwrap();
    content
}
fn sound(plan: &AdmittedReceivingTemplate, slot: ThingTemplateAudioType) -> Option<String> {
    plan.audio_event(slot)
        .map(|event| event.get_event_name().to_string())
}

#[test]
fn receiving_nosound_clears_inherited_and_map_solo_audio_without_affecting_peer_or_base() {
    use ThingTemplateAudioType::{SoundAmbient, SoundAmbientDamaged};
    let mut driving = owner("BaseEvent");
    let mut peer = owner("PeerEvent");
    let base = driving.admitted_template("AudioOwner").unwrap();
    assert_eq!(sound(&base, SoundAmbient).as_deref(), Some("BaseEvent"));
    assert_eq!(
        sound(&base, SoundAmbientDamaged).as_deref(),
        Some("DamageEvent")
    );
    assert_eq!(
        sound(
            &driving.admitted_template("QuietOwner").unwrap(),
            SoundAmbient
        ),
        None
    );
    driving
        .load_ini_text(
            "Object AudioOwner\nSoundAmbient = NOSOUND\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let map = driving.admitted_template("AudioOwner").unwrap();
    assert_eq!(sound(&map, SoundAmbient), None);
    assert_eq!(
        sound(&map, SoundAmbientDamaged).as_deref(),
        Some("DamageEvent")
    );
    assert_eq!(sound(&base, SoundAmbient).as_deref(), Some("BaseEvent"));
    assert_eq!(
        sound(&peer.admitted_template("AudioOwner").unwrap(), SoundAmbient).as_deref(),
        Some("PeerEvent")
    );
    driving
        .load_ini_text(
            "Object AudioOwner\nSoundAmbient = SoloEvent\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let solo = driving.admitted_template("AudioOwner").unwrap();
    assert_eq!(sound(&solo, SoundAmbient).as_deref(), Some("SoloEvent"));
    driving
        .load_ini_text(
            "Object AudioOwner\nSoundAmbient = noSoUnD = ignoredTrailingToken\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    assert_eq!(
        sound(
            &driving.admitted_template("AudioOwner").unwrap(),
            SoundAmbient
        ),
        None
    );
    assert_eq!(sound(&solo, SoundAmbient).as_deref(), Some("SoloEvent"));
    peer.reset_map_overrides();
    assert_eq!(
        sound(
            &driving.admitted_template("AudioOwner").unwrap(),
            SoundAmbient
        ),
        None
    );
    driving.reset_map_overrides();
    assert!(Arc::ptr_eq(
        &driving.admitted_template("AudioOwner").unwrap(),
        &base
    ));
    assert_eq!(
        sound(
            &driving.admitted_template("AudioOwner").unwrap(),
            SoundAmbient
        )
        .as_deref(),
        Some("BaseEvent")
    );
    assert_eq!(
        sound(
            &driving.admitted_template("QuietOwner").unwrap(),
            SoundAmbient
        ),
        None
    );
    assert_eq!(
        sound(&peer.admitted_template("AudioOwner").unwrap(), SoundAmbient).as_deref(),
        Some("PeerEvent")
    );
    let before_owner = driving.admitted_template("AudioOwner").unwrap();
    let before_quiet = driving.admitted_template("QuietOwner").unwrap();
    for missing in ["", " = = \t"] {
        let malformed = format!(
            "Object AudioOwner\nSoundAmbient = BeforeRejectedTail\nEnd\nObject QuietOwner\nSoundAmbient = {missing}\nEnd\n"
        );
        let error = driving
            .load_ini_text(&malformed, INILoadType::CreateOverrides)
            .unwrap_err();
        assert!(
            error.contains("Missing SoundAmbient audio event token"),
            "{error}"
        );
        assert!(Arc::ptr_eq(
            &driving.admitted_template("AudioOwner").unwrap(),
            &before_owner
        ));
        assert!(Arc::ptr_eq(
            &driving.admitted_template("QuietOwner").unwrap(),
            &before_quiet
        ));
        assert_eq!(
            sound(&before_owner, SoundAmbient).as_deref(),
            Some("BaseEvent")
        );
        assert_eq!(sound(&before_quiet, SoundAmbient), None);
        assert_eq!(
            sound(&peer.admitted_template("AudioOwner").unwrap(), SoundAmbient).as_deref(),
            Some("PeerEvent")
        );
    }
}
