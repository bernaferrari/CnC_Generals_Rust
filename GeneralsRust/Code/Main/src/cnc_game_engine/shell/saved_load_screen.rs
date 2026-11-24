//! A restored skirmish presents its saved GameInfo and receiving definitions.
use super::*;
use crate::skirmish_config::saved_admission::SavedSkirmishAdmission;
use crate::skirmish_config::{SkirmishPlayerTemplateSelection, SkirmishSlotConfig};
use game_client::gui::challenge_generals::ChallengeGenerals;
use game_client::gui::load_screen::{
    LoadScreenInitContext, LoadScreenKind, LoadScreenSlotInitContext,
    set_multiplayer_load_screen_general,
};
use game_engine::common::ini::ini_multiplayer::MultiplayerSettings;
use game_engine::common::rts::player_template::{PlayerTemplate, PlayerTemplateStore};
use game_engine::common::system::skirmish_game_info::SKIRMISH_MAX_SLOTS;

// GameInfo.h: original negative selections are distinct from resolved indices.
const RANDOM_SELECTION: i32 = -1;
const OBSERVER_SELECTION: i32 = -2;

fn saved_slot_template<'a>(
    slot: &SkirmishSlotConfig,
    store: Option<&'a PlayerTemplateStore>,
) -> Option<&'a PlayerTemplate> {
    let store = store?;
    match &slot.player_template {
        SkirmishPlayerTemplateSelection::Exact {
            template_index,
            template_name,
        } => store
            .get_nth_player_template_signed(*template_index)
            .filter(|entry| entry.name == *template_name),
        SkirmishPlayerTemplateSelection::Observer => store.find_template("FactionObserver"),
        _ => None,
    }
}

fn saved_skirmish_load_screen(
    admission: &SavedSkirmishAdmission,
    templates: Option<&PlayerTemplateStore>,
    settings: Option<&MultiplayerSettings>,
    generals: &ChallengeGenerals,
    mut text: impl FnMut(&str) -> String,
) -> LoadScreenInitContext {
    let local = admission.local_slot.and_then(|id| {
        admission
            .config
            .slots
            .iter()
            .find(|slot| slot.is_active && slot.slot_index == id as usize)
    });
    let local_id = local.map_or(-1, |slot| slot.slot_index as i32);
    let mut context = LoadScreenInitContext {
        local_player_name: local
            .map(|slot| slot.player_name.clone())
            .unwrap_or_default(),
        local_team_number: local_id,
        map_name: Some(admission.config.map.clone()),
        start_positions: vec![None; SKIRMISH_MAX_SLOTS],
        ..LoadScreenInitContext::default()
    };
    context.slots = admission
        .config
        .slots
        .iter()
        .filter(|slot| slot.is_active)
        .map(|slot| {
            let index = slot.slot_index;
            let original = admission.source.as_ref().map(|info| &info.slots[index]);
            let ally = local.is_some_and(|local| {
                local.slot_index == index
                    || (slot.team >= 0 && local.team == slot.team)
                    || matches!(
                        local.player_template,
                        SkirmishPlayerTemplateSelection::Observer
                    )
            });
            let observer = matches!(
                slot.player_template,
                SkirmishPlayerTemplateSelection::Observer
            );
            let hide_template =
                !ally && settings.is_some_and(|settings| settings.show_random_player_template);
            let side_name = if observer {
                text("GUI:Observer")
            } else if hide_template
                && original.is_some_and(|row| row.orig_player_template == RANDOM_SELECTION)
            {
                text("GUI:Random")
            } else {
                saved_slot_template(slot, templates)
                    .map(|template| template.get_display_name().to_string())
                    .unwrap_or_else(|| slot.faction.clone())
            };
            let start_pos =
                if !ally && settings.is_some_and(|settings| settings.show_random_start_pos) {
                    original.map_or(slot.start_position, |row| row.orig_start_pos)
                } else {
                    slot.start_position
                };
            if !observer {
                context.start_positions[index] = usize::try_from(start_pos).ok();
            }
            let color = if observer {
                Some(OBSERVER_SELECTION)
            } else {
                original
                    .map(|row| {
                        if !ally && settings.is_some_and(|settings| settings.show_random_color) {
                            row.orig_color
                        } else {
                            row.color
                        }
                    })
                    .or_else(|| {
                        admission
                            .config
                            .recorded_palette
                            .map(|colors| colors[index])
                    })
            };
            let (r, g, b) = slot.color_rgb;
            let text_color = color
                .and_then(|color| settings.and_then(|settings| settings.get_color_value(color)))
                .unwrap_or((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
            LoadScreenSlotInitContext {
                player_id: index as i32,
                player_name: slot.player_name.clone(),
                side_name,
                team_number: slot.team,
                apparent_color: color.filter(|color| *color >= 0),
                apparent_text_color: Some(text_color),
                is_ai: !slot.is_human,
                has_map: true,
                visible: true,
            }
        })
        .collect();
    context.slots.sort_unstable_by_key(|slot| slot.player_id);
    context.local_side_name = context
        .slots
        .iter()
        .find(|slot| slot.player_id == local_id)
        .map(|slot| slot.side_name.clone())
        .unwrap_or_default();
    set_multiplayer_load_screen_general(
        &mut context,
        local.and_then(|slot| saved_slot_template(slot, templates)),
        generals,
        text,
    );
    context
}

impl CnCGameEngine {
    /// The detached candidate is installed and outgoing audio has been reset.
    /// Queue the screen's destructor fade before saved ScriptEngine/Drawable audio.
    pub(in crate::cnc_game_engine) fn show_accepted_saved_load_screen(
        &mut self,
    ) -> Result<(), String> {
        let kind = self
            .select_cpp_load_screen(self.game_logic.game_mode(), true)
            .unwrap_or(LoadScreenKind::ShellGame);
        let context = if kind == LoadScreenKind::Multiplayer {
            self.game_logic
                .initial_skirmish_admission
                .as_ref()
                .map(|admission| {
                    let store = self.game_client.ensure_game_text_store();
                    let mut text = store.write().unwrap_or_else(|error| error.into_inner());
                    saved_skirmish_load_screen(
                        admission,
                        self.game_logic.player_template_definitions.as_ref(),
                        self.game_logic.multiplayer_definitions.as_deref(),
                        self.game_client.challenge_generals(),
                        |key| text.fetch_owned(key),
                    )
                })
                .unwrap_or_default()
        } else {
            LoadScreenInitContext::default()
        };
        self.initialize_shell_loading_overlay(kind, &context)?;
        let result = self.update_shell_loading_progress(1.0, Some("Restoring saved game"));
        self.hide_shell_loading_overlay();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::{INILoadType, ini_multiplayer::MultiplayerColorDefinition};
    use game_engine::common::system::skirmish_game_info::{SkirmishGameInfo, SkirmishSlot};

    #[test]
    fn saved_screen_uses_resolved_local_faction_and_apparent_saved_roster() {
        let mut templates = PlayerTemplateStore::new_owned();
        templates
            .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n\
             PlayerTemplate FactionGLA\n Side = GLA\n BaseSide = GLA\n StartingBuilding = GLACommandCenter\nEnd\n",
                INILoadType::Overwrite,
            )
            .unwrap();
        let america = templates.find_template_index("FactionAmerica").unwrap();
        let gla = templates.find_template_index("FactionGLA").unwrap();
        templates
            .get_nth_player_template_mut(america)
            .unwrap()
            .load_screen_music = "Load_USA".into();
        templates
            .get_nth_player_template_mut(gla)
            .unwrap()
            .load_screen_music = "Load_GLA".into();
        let mut settings = MultiplayerSettings::new();
        for value in [0x123456, 0x234567] {
            let mut color = MultiplayerColorDefinition::new(format!("Color {value}").into());
            color.set_color(value);
            settings.color_definitions.push(color);
        }
        let mut source = SkirmishGameInfo {
            map_name: "SavedSkirmish.map".into(),
            in_game: true,
            in_progress: true,
            ..Default::default()
        };
        source.slots[2] = SkirmishSlot {
            state: 2,
            name: "Earlier AI".into(),
            player_template: america as i32,
            orig_player_template: RANDOM_SELECTION,
            start_pos: 1,
            orig_start_pos: -1,
            color: 0,
            orig_color: RANDOM_SELECTION,
            team_number: -1,
            ..Default::default()
        };
        source.slots[6] = SkirmishSlot {
            state: 5,
            name: "Saved local".into(),
            player_template: gla as i32,
            orig_player_template: gla as i32,
            start_pos: 3,
            orig_start_pos: 3,
            color: 1,
            orig_color: 1,
            team_number: -1,
            ..Default::default()
        };
        let admission = SavedSkirmishAdmission::from_original_with_settings(
            source,
            &templates,
            Some(&settings),
        )
        .unwrap();
        let generals = ChallengeGenerals::default();
        let context = saved_skirmish_load_screen(
            &admission,
            Some(&templates),
            Some(&settings),
            &generals,
            |key| key.to_string(),
        );
        assert_eq!(context.local_player_name, "Saved local");
        assert_eq!(context.local_team_number, 6);
        assert_eq!(context.local_template_name, "FactionGLA");
        assert_eq!(context.local_load_screen_music, "Load_GLA");
        assert_eq!(context.map_name.as_deref(), Some("SavedSkirmish.map"));
        assert_eq!(context.slots[0].side_name, "GUI:Random");
        assert_eq!(context.slots[0].apparent_color, None);
        assert_eq!(context.slots[1].apparent_color, Some(1));
        assert_eq!(context.start_positions[2], None);
        assert_eq!(context.start_positions[6], Some(3));

        let mut no_local = admission;
        no_local.local_slot = None;
        let context = saved_skirmish_load_screen(
            &no_local,
            Some(&templates),
            Some(&settings),
            &generals,
            |key| key.to_string(),
        );
        assert_eq!(context.local_team_number, -1);
        assert!(context.local_load_screen_music.is_empty());
    }
}
