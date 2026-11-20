//! Ordered application startup content admission, before UI and map workers.
use super::*;

const MULTIPLAYER_PATHS: &[&str] = &[
    "Data/INI/Default/Multiplayer.ini",
    "Data/INI/Multiplayer.ini",
];

impl CnCGameEngine {
    pub(super) async fn load_startup_ini_texts(
        paths: &[&'static str],
        application_assets: Option<&Mutex<crate::assets::manager::AssetManager>>,
    ) -> Result<Vec<(&'static str, String)>, String> {
        if let Some(assets) = application_assets {
            let mut assets = assets
                .lock()
                .map_err(|_| "asset manager poisoned".to_string())?;
            return assets.read_ordered_ini_sources(paths, Self::read_startup_ini_from_disk);
        }

        // Before the application service exists, startup has its own archive
        // reader. It must never adopt another application's published manager.
        let mut texts = Vec::new();
        let mut startup_archive = None;
        for &path in paths {
            let text = if let Some(text) = Self::read_startup_ini_from_disk(path) {
                Some(text)
            } else {
                if startup_archive.is_none() {
                    let mut archive = crate::assets::archive::ArchiveFileSystem::new();
                    archive
                        .init()
                        .await
                        .map_err(|error| format!("startup archive admission: {error}"))?;
                    startup_archive = Some(archive);
                }
                match startup_archive
                    .as_mut()
                    .expect("local archive initialized")
                    .open_file_sync(path)
                {
                    Ok(bytes) => {
                        Some(String::from_utf8(bytes).map_err(|error| format!("{path}: {error}"))?)
                    }
                    Err(_) => None,
                }
            };
            if let Some(text) = text {
                texts.push((path, text));
            }
        }
        Ok(texts)
    }

    /// Load ordered startup sources into a private headless setup.
    pub(crate) async fn load_owned_multiplayer_definitions()
    -> Result<game_engine::common::ini::ini_multiplayer::MultiplayerSettings, String> {
        let texts = Self::load_startup_ini_texts(MULTIPLAYER_PATHS, None).await?;
        let mut settings = game_engine::common::ini::ini_multiplayer::MultiplayerSettings::new();
        for (path, text) in texts {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(&text, |ini| {
                ini.parse_current_file_into_multiplayer_settings(&mut settings)
            })
            .map_err(|err| format!("{path}: {err}"))?;
        }
        if settings.color_definitions.is_empty() {
            return Err(
                "authored MultiplayerColorDefinition palette is unavailable at startup".into(),
            );
        }
        Ok(settings)
    }

    pub(super) async fn load_startup_multiplayer_definitions(
        application_assets: &Mutex<crate::assets::manager::AssetManager>,
    ) -> Result<i32, String> {
        let texts =
            Self::load_startup_ini_texts(MULTIPLAYER_PATHS, Some(application_assets)).await?;
        admit_multiplayer_texts(&texts)
    }
}

fn admit_multiplayer_texts(texts: &[(&str, String)]) -> Result<i32, String> {
    // C++ GameEngine.cpp:399: Default first, then authored override. The boot
    // caller owns this phase; no mutable loaded flag or map-worker re-admission.
    for (path, text) in texts {
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(text, |ini| ini.parse_current_file())
            .map_err(|err| format!("{path}: {err}"))?;
        info!("Loaded startup setup definitions from {}", path);
    }
    let has_palette =
        game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
            !settings.color_definitions.is_empty()
        });
    if !has_palette {
        return Err("authored MultiplayerColorDefinition palette is unavailable at startup".into());
    }
    Ok(
        game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
            settings.max_beacons_per_player.max(0)
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "game_client")]
    #[test]
    fn mounted_campaign_definitions_initialize_the_receiving_client() {
        use game_engine::common::ini::ini_campaign::CampaignStore;

        let archive = crate::fixture_content::retail_content_root()
            .join("Command & Conquer Generals Zero Hour/Data/INIZH.big");
        let mut files = crate::assets::archive::ArchiveFileSystem::new();
        files.set_local_search_roots(Vec::new());
        futures::executor::block_on(files.load_big_file(archive))
            .expect("mount Zero Hour INIZH.big");
        let bytes = files
            .read_optional_definition_source("Data/INI/Campaign.ini")
            .unwrap()
            .expect("mounted Campaign.ini");
        let definitions = CampaignStore::from_ini_text(&String::from_utf8(bytes).unwrap())
            .expect("parse mounted Campaign.ini");
        let mut receiving = game_client::core::GameClient::new().unwrap();
        receiving.mark_initialized_with_campaign_definitions(&definitions);
        assert!(receiving.with_window_event_effects(None, |effects| {
            game_client::gui::simulate_main_menu_campaign_start_residual(
                game_client::gui::ShowSide::USA,
                game_client::gui::GameDifficulty::Hard,
                effects,
            )
        }));
        let map = receiving.campaign_manager().get_current_map().unwrap();
        assert_eq!(map, "Maps\\MD_USA01\\MD_USA01.map");
        assert_eq!(receiving.campaign_manager().get_game_difficulty() as i32, 2);

        let mut peer = game_client::core::GameClient::new().unwrap();
        let other = CampaignStore::from_ini_text(
            "Campaign USA\n FirstMission Mission01\n Mission Mission01\n Map Maps\\Fixture\\Fixture.map\n End\nEnd\n",
        ).unwrap();
        peer.mark_initialized_with_campaign_definitions(&other);
        peer.campaign_manager_mut().set_campaign("USA");
        assert_eq!(peer.campaign_manager().get_current_map().as_deref(), Some("Maps\\Fixture\\Fixture.map"));
        assert_eq!(
            receiving.campaign_manager().get_current_map().as_deref(),
            Some(map.as_str())
        );
        assert_eq!(
            receiving.campaign_manager().get_next_mission().unwrap().map_name,
            "Maps\\MD_USA02\\MD_USA02.map"
        );
    }

    #[test]
    fn ordered_multiplayer_admission_initializes_actual_menu_without_duplicate_money() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "ordered_multiplayer_admission_initializes_actual_menu_without_duplicate_money",
            || {
                use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
                let (first, money_before) = with_multiplayer_settings(|settings| {
                    (
                        settings.get_num_colors(),
                        settings.starting_money_choices.len(),
                    )
                });
                let texts = [
                    (MULTIPLAYER_PATHS[0], "MultiplayerColor StartupOrderedChoice\n RGBColor = R:1 G:2 B:3\n RGBNightColor = R:4 G:5 B:6\nEnd\nMultiplayerStartingMoneyChoice\n Value = 10000\n Default = Yes\nEnd\n".to_string()),
                    (MULTIPLAYER_PATHS[1], "MultiplayerColor StartupOrderedChoice\n RGBColor = R:7 G:8 B:9\n RGBNightColor = R:10 G:11 B:12\nEnd\n".to_string()),
                ];
                admit_multiplayer_texts(&texts).unwrap();
                with_multiplayer_settings(|settings| {
                    assert_eq!(settings.get_num_colors(), first + 1);
                    assert_eq!(settings.get_color_value(first), Some(0xFF07_0809));
                    assert_eq!(settings.starting_money_choices.len(), money_before + 1);
                });
                let mut menu = crate::ui::skirmish_menu::SkirmishMenu::new();
                menu.initialize().unwrap();
                assert!(menu.select_slot_color(0, first));
                // Menu capture is a read; neither UI nor the map worker parses it again.
                with_multiplayer_settings(|settings| {
                    assert_eq!(settings.starting_money_choices.len(), money_before + 1);
                });
            },
        );
    }
}
