//! Ordered application startup content admission, before UI and map workers.
use super::*;

const MULTIPLAYER_PATHS: &[&str] = &[
    "Data/INI/Default/Multiplayer.ini",
    "Data/INI/Multiplayer.ini",
];

impl CnCGameEngine {
    pub(super) async fn load_startup_ini_texts(
        paths: &[&'static str],
    ) -> Result<Vec<(&'static str, String)>, String> {
        let mut texts = Vec::new();
        for &path in paths {
            let text = if let Some(text) = Self::read_startup_ini_from_disk(path) {
                Some(text)
            } else {
                if let Some(manager) = crate::assets::manager::get_asset_manager() {
                    let mut manager = manager
                        .lock()
                        .map_err(|_| "asset manager poisoned".to_string())?;
                    match manager.extract_file(path).await {
                        Ok(bytes) => {
                            Some(String::from_utf8(bytes).map_err(|err| format!("{path}: {err}"))?)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                }
            };
            if let Some(text) = text {
                texts.push((path, text));
            }
        }
        Ok(texts)
    }

    pub(super) async fn load_startup_multiplayer_definitions() -> Result<(), String> {
        let texts = Self::load_startup_ini_texts(MULTIPLAYER_PATHS).await?;
        admit_multiplayer_texts(&texts)
    }
}

fn admit_multiplayer_texts(texts: &[(&str, String)]) -> Result<(), String> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
