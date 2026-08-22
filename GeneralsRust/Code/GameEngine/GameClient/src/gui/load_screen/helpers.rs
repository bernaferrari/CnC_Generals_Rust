// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MultiplayerLocalGeneralPresentation {
    template_name: String,
    name: String,
    features: String,
    portrait: Option<String>,
    load_screen_music: String,
}

fn multiplayer_local_general_presentation(
    template: Option<&PlayerTemplate>,
    fallback_side_name: &str,
) -> MultiplayerLocalGeneralPresentation {
    let Some(template) = template else {
        return MultiplayerLocalGeneralPresentation {
            template_name: fallback_side_name.to_string(),
            name: fallback_side_name.to_string(),
            features: fallback_side_name.to_string(),
            portrait: None,
            load_screen_music: String::new(),
        };
    };

    let mut presentation = MultiplayerLocalGeneralPresentation {
        template_name: template.name.clone(),
        name: template.get_display_name().to_string(),
        features: if template.features.is_empty() {
            GameText::fetch("GUI:PlayerObserver")
        } else {
            GameText::fetch(&template.features)
        },
        portrait: None,
        load_screen_music: template.load_screen_music.clone(),
    };

    if let Some(generals) = get_challenge_generals() {
        if let Ok(generals) = generals.lock() {
            if let Some(general) = generals.general_by_template_name(&template.name) {
                presentation.name = GameText::fetch(general.bio_name());
                presentation.portrait = general.bio_portrait_large().map(str::to_string);
            }
        }
    }

    presentation
}

fn multiplayer_local_general_text_fallback<'a>(text: &'a str, fallback: &'a str) -> &'a str {
    if text.is_empty() {
        fallback
    } else {
        text
    }
}

fn play_multiplayer_load_screen_music(music_name: &str) {
    if music_name.is_empty() {
        return;
    }
    #[cfg(not(test))]
    {
        if let Some(audio) = TheAudio::get() {
            // C++ LoadScreen.cpp:1351 AHSV_StopTheMusicFade first.
            audio.remove_audio_event(game_engine::common::audio::AHSV_STOP_THE_MUSIC_FADE);
            let mut event = AudioEventRts::new(music_name);
            event.set_should_fade(true);
            let _ = audio.add_audio_event(&event);
            audio.update();
        }
    }
    #[cfg(test)]
    {
        let _ = music_name;
    }
}

fn set_progress_window(wm: &mut WindowManager, name: &str, percent: f32) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().send_system_message(
            WindowMessage::User(GPM_SET_PROGRESS),
            (percent as i32) as WindowMsgData,
            0,
        );
    }
}

fn set_progress_window_fill_color(wm: &mut WindowManager, name: &str, color: u32) {
    if let Some(window) = wm.find_window_by_name(name) {
        if let Some(progress) = window.borrow_mut().progress_bar_mut() {
            progress.set_fill_color(color_u32_to_gadget_color(color));
        }
    }
}

fn color_u32_to_gadget_color(color: u32) -> crate::gui::gadgets::Color {
    crate::gui::gadgets::Color::rgba(
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
        ((color >> 24) & 0xff) as u8,
    )
}

fn set_window_text(wm: &mut WindowManager, name: &str, text: &str) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().set_text(text);
    }
}

fn set_window_enabled_text_color(wm: &mut WindowManager, name: &str, color: u32) {
    if let Some(window) = wm.find_window_by_name(name) {
        let border_color = window.borrow().get_enabled_text_border_color();
        window
            .borrow_mut()
            .set_enabled_text_colors(color, border_color);
    }
}

fn set_window_image(
    wm: &mut WindowManager,
    window_name: &str,
    image_index: usize,
    image_name: &str,
    mark_image_status: bool,
) {
    let mut image = WindowImage {
        name: image_name.to_string(),
        width: 0,
        height: 0,
    };
    if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            image.width = found.get_image_width();
            image.height = found.get_image_height();
        }
    }

    if let Some(window) = wm.find_window_by_name(window_name) {
        let mut window = window.borrow_mut();
        if window.set_enabled_image(image_index, image).is_ok() && mark_image_status {
            window.set_status(WindowStatus::IMAGE);
        }
    }
}

fn hide_window(wm: &mut WindowManager, name: &str, hidden: bool) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().hide(hidden);
    }
}

fn single_player_campaign_images(campaign_name: &str) -> Option<(&'static str, &'static str)> {
    if campaign_name.eq_ignore_ascii_case("USA") {
        Some(("MissionLoad_USA", "LoadingBar_ProgressCenter2"))
    } else if campaign_name.eq_ignore_ascii_case("GLA") {
        Some(("MissionLoad_GLA", "LoadingBar_ProgressCenter3"))
    } else if campaign_name.eq_ignore_ascii_case("China") {
        Some(("MissionLoad_China", "LoadingBar_ProgressCenter1"))
    } else {
        None
    }
}

fn single_player_mission_text(mission: &Mission) -> SinglePlayerMissionText {
    SinglePlayerMissionText {
        objective_lines: mission.mission_objectives_label.each_ref().map(|label| {
            if label.is_empty() {
                String::new()
            } else {
                GameText::fetch(label)
            }
        }),
        unit_descriptions: mission
            .unit_names
            .each_ref()
            .map(|label| GameText::fetch(label)),
        location: GameText::fetch(&mission.location_name_label),
    }
}

fn challenge_persona_text(persona: &GeneralPersona) -> ChallengePersonaText {
    let name = GameText::fetch(persona.bio_name());
    ChallengePersonaText {
        big_name: name.clone(),
        name,
        rank: GameText::fetch(persona.bio_rank()),
        strategy: GameText::fetch(persona.bio_strategy()),
        portrait_large: persona.bio_portrait_large().map(str::to_string),
        portrait_movie_left: persona.portrait_movie_left_name().to_string(),
        portrait_movie_right: persona.portrait_movie_right_name().to_string(),
        name_sound: persona.name_sound().to_string(),
        taunt_sounds: [
            persona.taunt_sound_1().to_string(),
            persona.taunt_sound_2().to_string(),
            persona.taunt_sound_3().to_string(),
        ],
    }
}

fn challenge_persona_text_for_current_mission(
    campaign_name: &str,
    mission_general_name: &str,
    generals: &ChallengeGenerals,
) -> Option<(ChallengePersonaText, ChallengePersonaText)> {
    let player = generals.player_general_by_campaign_name(campaign_name)?;
    let opponent = generals.general_by_general_name(mission_general_name)?;
    Some((
        challenge_persona_text(player),
        challenge_persona_text(opponent),
    ))
}

fn current_challenge_persona_text() -> Option<(ChallengePersonaText, ChallengePersonaText)> {
    let campaign_manager = get_campaign_manager();
    let campaign = campaign_manager.get_current_campaign()?;
    let mission = campaign_manager.get_current_mission()?;
    if get_challenge_generals().is_none() {
        init_challenge_generals();
    }
    let generals = get_challenge_generals()?;
    let generals = generals.lock().ok()?;
    challenge_persona_text_for_current_mission(&campaign.name, &mission.general_name, &generals)
}

fn current_challenge_movie_label() -> Option<String> {
    let campaign_manager = get_campaign_manager();
    let mission = campaign_manager.get_current_mission()?;
    let movie_label = mission.movie_label.trim();
    (!movie_label.is_empty()).then(|| movie_label.to_string())
}

fn current_challenge_voice_length() -> i32 {
    let campaign_manager = get_campaign_manager();
    campaign_manager
        .get_current_mission()
        .map(|mission| mission.voice_length)
        .unwrap_or(0)
}
