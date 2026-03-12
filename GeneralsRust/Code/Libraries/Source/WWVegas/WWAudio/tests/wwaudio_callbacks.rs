use std::sync::{Arc, Mutex};

use wp_audio::{device::EndOfStreamCallback, wwaudio::WWAudioClass, SoundObjectId};

#[test]
fn text_callbacks_fire_without_audio_system() {
    let mut ww_audio = WWAudioClass::new();
    let received = Arc::new(Mutex::new(Vec::<String>::new()));

    let text_callback = {
        let sink = Arc::clone(&received);
        Arc::new(move |text: &str| {
            sink.lock().unwrap().push(text.to_string());
        })
    };

    ww_audio.Register_Text_Callback(text_callback);

    ww_audio.Fire_Text_Callback("hello_world");
    let entries = received.lock().unwrap();
    assert_eq!(entries.as_slice(), &["hello_world"]);
}

#[test]
fn eos_callbacks_are_cached_and_unregistered() {
    let mut ww_audio = WWAudioClass::new();
    let invoked = Arc::new(Mutex::new(Vec::<u32>::new()));

    let eos_callback: EndOfStreamCallback = {
        let sink = Arc::clone(&invoked);
        Arc::new(move |handle_id, _| {
            sink.lock().unwrap().push(handle_id);
        })
    };

    ww_audio.Register_EOS_Callback(Arc::clone(&eos_callback), 99);
    ww_audio.UnRegister_EOS_Callback(&eos_callback);
    assert!(invoked.lock().unwrap().is_empty());
}

#[test]
fn logical_definitions_apply_before_initialization() {
    let mut ww_audio = WWAudioClass::new();
    let text_events = Arc::new(Mutex::new(Vec::<String>::new()));

    let text_callback = {
        let sink = Arc::clone(&text_events);
        Arc::new(move |text: &str| {
            sink.lock().unwrap().push(text.to_string());
        })
    };
    ww_audio.Register_Text_Callback(text_callback);

    let logical_id: SoundObjectId = 1;
    ww_audio.Register_Logical_Sound_Definition(logical_id, 0xA5, Some("LogicalAlert"));

    let logical_sound = ww_audio.Create_Logical_Sound();
    assert_eq!(logical_sound.type_mask(), 0xA5);

    let events = text_events.lock().unwrap();
    assert_eq!(events.as_slice(), &["LogicalAlert"]);
}
