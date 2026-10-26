//! Source InGameUI.cpp2256-2459 observations from this exact Main world.
use super::*;
impl CnCGameEngine {
    pub(super) fn receiving_mouseover_facts(
        &self,
        hover: Option<crate::game_logic::ObjectId>,
    ) -> Result<game_client::gui::ingame_ui::ReceivingMouseoverFacts, String> {
        use game_client::gui::ingame_ui::{
            MouseoverObjectObservation, MouseoverPlayerObservation, ReceivingMouseoverFacts,
        };
        let mut facts = ReceivingMouseoverFacts {
            multiplayer: self
                .game_logic
                .replay_pending
                .recorder_owned()
                .is_multiplayer_for_game_mode(match self.game_logic.game_mode() {
                    crate::game_logic::GameMode::SinglePlayer => 0,
                    crate::game_logic::GameMode::Skirmish => 2,
                    crate::game_logic::GameMode::Replay => 3,
                    crate::game_logic::GameMode::Shell => 4,
                    crate::game_logic::GameMode::None => 6,
                    crate::game_logic::GameMode::Internet => 5,
                    _ => 1,
                }),
            objects: Default::default(),
        };
        let Some(hover) = hover else {
            return Ok(facts);
        };
        let local_id = self
            .game_logic
            .local_player_id()
            .ok_or("native hover has no receiving local player")?;
        let local = self
            .game_logic
            .get_players()
            .get(&local_id)
            .ok_or("native hover local player absent")?;
        let ui = self
            .game_client
            .in_game_ui_subsystem()
            .ok_or("native hover has no receiving UI")?;
        let entry = ui
            .presentation_unit_catalog()
            .iter()
            .find(|entry| entry.object_id == hover.0)
            .ok_or("native hover object absent from receiving presentation")?;
        let mut ids = vec![hover.0];
        if entry
            .kind_names
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case("IgnoredInGui"))
        {
            if let Some(id) = entry.slaver_object_id {
                ids.push(id);
            }
        }
        for id in ids {
            let object = self
                .game_logic
                .get_object(crate::game_logic::ObjectId(id))
                .ok_or("native hover object retired before observation")?;
            let player = object
                .owner_player_id
                .map(|id| {
                    self.game_logic
                        .get_players()
                        .get(&id)
                        .ok_or("native hover controlling player absent")
                })
                .transpose()?;
            let allied = player.is_some_and(|player| {
                player.id == local_id
                    || self.game_logic.player_relationship(player.id, local_id)
                        == gamelogic::common::Relationship::Allies
            });
            let friendly = allied || !local.is_alive || local.is_observer;
            let hidden_garrison = player.is_some()
                && object
                    .building_data
                    .as_ref()
                    .is_some_and(|data| data.hide_garrisoned_state)
                && !allied;
            let apparent_id = if hidden_garrison {
                let data = object.building_data.as_ref().expect("checked building");
                if data.original_team.is_none() {
                    return Err("hidden garrison lacks original owner admission".into());
                }
                data.original_garrison_player_id
            } else if object.status.disguised && !friendly {
                object.disguise_as_player_id
            } else {
                object.owner_player_id
            };
            let player = apparent_id
                .map(|id| {
                    self.game_logic
                        .get_players()
                        .get(&id)
                        .ok_or("native hover apparent player absent")
                })
                .transpose()?;
            let owner = player
                .map(|player| {
                    let template = self
                        .game_logic
                        .resolved_player_template(player.id)
                        .ok_or("native hover player has no receiving PlayerTemplate")?;
                    let (r, g, b) = player.color_rgb;
                    Ok::<_, String>(MouseoverPlayerObservation {
                        display_name: player.name.clone(),
                        playable_side: template.is_playable_side(),
                        color: [r, g, b, 255],
                    })
                })
                .transpose()?;
            let mut names = std::collections::HashMap::new();
            let template = if object.status.disguised && !friendly {
                let name = object
                    .disguise_as_template
                    .as_ref()
                    .ok_or("disguised hover lacks copied template")?;
                self.game_logic
                    .get_templates()
                    .get(name)
                    .ok_or("disguised hover receiving template absent")?
            } else {
                object.get_template()
            };
            let raw = &template.display_name;
            let name = if raw.contains(':') {
                game_client::game_text::GameText::fetch(raw)
            } else {
                raw.clone()
            };
            names.insert(template.name.clone(), name);
            let projected = ui
                .presentation_unit_catalog()
                .iter()
                .find(|entry| entry.object_id == id)
                .ok_or("hover slaver absent from receiving presentation")?;
            let apparent_name = if projected.disguised && !friendly {
                projected
                    .disguise_as_template
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&projected.template_name)
            } else {
                &projected.template_name
            };
            if !names.contains_key(apparent_name) {
                return Err(
                    "native hover presentation association changed before observation".into(),
                );
            }

            facts.objects.insert(
                id,
                MouseoverObjectObservation {
                    visible_friendly: friendly,
                    player: owner,
                    display_names: names,
                },
            );
        }
        Ok(facts)
    }
}
