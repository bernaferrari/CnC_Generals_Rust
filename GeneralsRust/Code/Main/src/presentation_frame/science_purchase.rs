//! Freeze ControlBar.cpp's local-player purchase rows from one receiving match.
use game_client::gui::control_bar::{
    CommandOption,
    control_bar::{
        MAX_PURCHASE_SCIENCE_RANK_1, MAX_PURCHASE_SCIENCE_RANK_3, MAX_PURCHASE_SCIENCE_RANK_8,
        ScienceButtonState,
    },
};
use game_engine::common::rts::science::{
    SCIENCE_INVALID, ScienceAccess, ScienceNameFacts, ScienceType,
};

use crate::game_logic::{GameLogic, Player};

struct PlayerScienceAccess<'a> {
    player: &'a Player,
    facts: &'a ScienceNameFacts,
}

impl ScienceAccess for PlayerScienceAccess<'_> {
    fn has_science(&self, science: ScienceType) -> bool {
        self.facts
            .name(science)
            .is_some_and(|name| self.player.has_unlocked_science(name))
    }
}

fn hidden_button() -> ScienceButtonState {
    ScienceButtonState {
        command_name: String::new(),
        science_type: SCIENCE_INVALID,
        is_hidden: true,
        is_enabled: false,
        is_purchased: false,
    }
}

pub(super) fn freeze(logic: &GameLogic, player_id: u32) -> [Vec<ScienceButtonState>; 3] {
    let Some(content) = logic.receiving_module_content.as_ref() else {
        return Default::default();
    };
    let Some(player) = logic.get_player(player_id) else {
        return Default::default();
    };
    let Some(template) = logic
        .player_template_definitions
        .as_ref()
        .and_then(|definitions| {
            logic
                .player_template_identity(player_id)?
                .resolve_in(definitions)
        })
    else {
        return Default::default();
    };
    let names = [
        template.get_purchase_science_command_set_rank1(),
        template.get_purchase_science_command_set_rank3(),
        template.get_purchase_science_command_set_rank8(),
    ];
    if names.iter().any(|name| name.is_empty()) {
        return Default::default();
    }
    let actions = content.action_definitions();
    let find_set = |name: &str| {
        actions.sets.find_command_set_resolved(name).or_else(|| {
            actions
                .sets
                .find_command_set_resolved(&name.to_ascii_uppercase())
        })
    };
    let (Some(rank1), Some(rank3), Some(rank8)) =
        (find_set(names[0]), find_set(names[1]), find_set(names[2]))
    else {
        return Default::default();
    };
    let facts = content.science_facts_ref();
    let sciences = facts.definitions();
    let access = PlayerScienceAccess { player, facts };
    let freeze_set = |set: &game_engine::common::ini::ini_command_set::CommandSet, limit: usize| {
        (0..limit)
            .map(|slot| {
                let Some(button) = set
                    .buttons
                    .get(slot)
                    .and_then(Option::as_ref)
                    .and_then(|name| actions.buttons.find_command_button_resolved(name))
                else {
                    return hidden_button();
                };
                if button.options_bits & CommandOption::ScriptOnly as u32 != 0 {
                    return hidden_button();
                }
                let science = button
                    .parsed_science_required
                    .first()
                    .copied()
                    .unwrap_or(SCIENCE_INVALID);
                let mut state = ScienceButtonState {
                    command_name: button.name.clone(),
                    science_type: science,
                    is_hidden: false,
                    is_enabled: false,
                    is_purchased: false,
                };
                let Some(name) = facts.name(science) else {
                    return state;
                };
                if player.is_science_disabled(name) {
                    return state;
                }
                if player.is_science_hidden(name) {
                    state.is_hidden = true;
                    return state;
                }
                state.is_purchased = player.has_unlocked_science(name);
                state.is_enabled = !state.is_purchased
                    && sciences.player_has_prereqs_for_science(&access, science)
                    && sciences.get_science_purchase_cost(science)
                        <= player.science_purchase_points;
                state.is_hidden = !sciences.player_has_root_prereqs_for_science(&access, science);
                state
            })
            .collect()
    };
    [
        freeze_set(rank1, MAX_PURCHASE_SCIENCE_RANK_1),
        freeze_set(rank3, MAX_PURCHASE_SCIENCE_RANK_3),
        freeze_set(rank8, MAX_PURCHASE_SCIENCE_RANK_8),
    ]
}

/// Capture each player's science membership and template facts once; the
/// selected object later identifies which record the command bar observes.
pub(super) fn freeze_selected_owners(
    logic: &GameLogic,
) -> std::collections::HashMap<u32, game_client::gui::control_bar::control_bar::SelectedPlayerFacts>
{
    let content = logic.receiving_module_content.as_ref();
    logic
        .get_players()
        .iter()
        .map(|(&id, player)| {
            let mut science_ids = player
                .unlocked_sciences
                .iter()
                .filter_map(|name| content.and_then(|content| content.science_type(name)))
                .collect::<Vec<_>>();
            science_ids.sort_unstable();
            science_ids.dedup();
            let template = logic
                .player_template_definitions
                .as_ref()
                .and_then(|store| logic.player_template_identity(id)?.resolve_in(store));
            let purchase_command_sets = template
                .map(|template| {
                    [
                        template.get_purchase_science_command_set_rank1().to_owned(),
                        template.get_purchase_science_command_set_rank3().to_owned(),
                        template.get_purchase_science_command_set_rank8().to_owned(),
                    ]
                })
                .unwrap_or_default();
            (
                id,
                game_client::gui::control_bar::control_bar::SelectedPlayerFacts {
                    science_ids,
                    purchase_command_sets,
                    beacon_template: template
                        .map(|template| template.get_beacon_template().to_owned())
                        .unwrap_or_default(),
                },
            )
        })
        .collect()
}
