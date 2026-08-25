//! GUIUtil.cpp / GUIUtil.h — lobby slot, color, team, and cash helpers.
//!
//! GameNetwork cannot depend on GameClient `GameWindow`, so these helpers
//! produce C++-matching combo/slot data that LAN/WOL menus apply to gadgets.

use crate::game_info::{
    GameInfo, MAX_SLOTS, Money, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM, lookup_game_text,
    lookup_multiplayer_settings,
};
use std::sync::atomic::{AtomicBool, Ordering};

static SLOT_LIST_UPDATES_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable slot-list gadget refreshes (C++ `EnableSlotListUpdates`).
pub fn enable_slot_list_updates(enabled: bool) {
    SLOT_LIST_UPDATES_ENABLED.store(enabled, Ordering::SeqCst);
}

/// C++ `AreSlotListUpdatesEnabled`.
pub fn are_slot_list_updates_enabled() -> bool {
    SLOT_LIST_UPDATES_ENABLED.load(Ordering::SeqCst)
}

/// One combo-box row matching `GadgetComboBoxAddEntry` + `SetItemData`.
#[derive(Debug, Clone)]
pub struct ComboEntry {
    pub label: String,
    pub data: i32,
    pub color: u32,
}

/// Per-slot accept / start-position enablement (C++ `EnableAcceptControls`).
#[derive(Debug, Clone, Default)]
pub struct SlotControlEnablement {
    pub player_combo: bool,
    pub color_combo: bool,
    pub template_combo: bool,
    pub team_combo: bool,
    pub accept_button: bool,
    pub start_position: bool,
}

/// Snapshot used by `UpdateSlotList` consumers.
#[derive(Debug, Clone)]
pub struct SlotListRow {
    pub slot_index: usize,
    pub occupied: bool,
    pub is_player: bool,
    pub is_ai: bool,
    pub is_local: bool,
    pub is_observer: bool,
    pub accepted: bool,
    pub name: String,
    pub color: i32,
    pub player_template: i32,
    pub team: i32,
    pub start_pos: i32,
    pub enablement: SlotControlEnablement,
}

fn random_or_first_color() -> u32 {
    lookup_multiplayer_settings()
        .and_then(|s| {
            s.random_color
                .map(|c| c as u32)
                .or_else(|| s.color_values.first().copied().map(|c| c as u32))
        })
        .unwrap_or(0x00FFFFFF)
}

/// C++ `PopulateColorComboBox`.
pub fn populate_color_combo_box(
    combo_slot: usize,
    game: &GameInfo,
    is_observer: bool,
) -> Vec<ComboEntry> {
    let settings = lookup_multiplayer_settings();
    let color_values = settings
        .as_ref()
        .map(|s| s.color_values.clone())
        .unwrap_or_default();
    let num_colors = color_values.len().max(8);
    let random_color = random_or_first_color();

    let mut available = vec![true; num_colors];
    for i in 0..MAX_SLOTS {
        if i == combo_slot {
            continue;
        }
        if let Some(slot) = game.get_slot(i) {
            let color = slot.get_color();
            if color >= 0 && (color as usize) < num_colors {
                available[color as usize] = false;
            }
        }
    }

    let mut entries = Vec::new();
    let none_label = if is_observer {
        lookup_game_text("GUI:None")
    } else {
        lookup_game_text("GUI:???")
    };
    entries.push(ComboEntry {
        label: none_label,
        data: -1,
        color: random_color,
    });
    if is_observer {
        return entries;
    }

    for (idx, taken) in available.iter().enumerate() {
        if !*taken {
            continue;
        }
        let color = color_values
            .get(idx)
            .copied()
            .map(|c| c as u32)
            .unwrap_or(0x00FFFFFF);
        entries.push(ComboEntry {
            label: lookup_game_text(&format!("Color:{idx}")),
            data: idx as i32,
            color,
        });
    }
    entries
}

/// C++ `PopulatePlayerTemplateComboBox`.
pub fn populate_player_template_combo_box(
    _combo_slot: usize,
    game: &GameInfo,
    allow_observers: bool,
    template_sides: &[(i32, String, bool)],
) -> Vec<ComboEntry> {
    let random_color = random_or_first_color();
    let mut entries = vec![ComboEntry {
        label: lookup_game_text("GUI:Random"),
        data: PLAYERTEMPLATE_RANDOM,
        color: random_color,
    }];

    let mut seen_sides = std::collections::BTreeSet::new();
    for (index, side, is_old_faction) in template_sides {
        if game.old_factions_only() && !*is_old_faction {
            continue;
        }
        let side_key = format!("SIDE:{side}");
        if !seen_sides.insert(side_key.clone()) {
            continue;
        }
        entries.push(ComboEntry {
            label: lookup_game_text(&side_key),
            data: *index,
            color: random_color,
        });
    }

    if allow_observers {
        entries.push(ComboEntry {
            label: lookup_game_text("GUI:Observer"),
            data: PLAYERTEMPLATE_OBSERVER,
            color: random_color,
        });
    }
    entries
}

/// C++ `PopulateTeamComboBox`.
pub fn populate_team_combo_box(
    _combo_slot: usize,
    _game: &GameInfo,
    is_observer: bool,
) -> Vec<ComboEntry> {
    let random_color = random_or_first_color();
    let mut entries = vec![ComboEntry {
        label: lookup_game_text("Team:0"),
        data: -1,
        color: random_color,
    }];
    if is_observer {
        return entries;
    }
    let num_teams = MAX_SLOTS / 2;
    for team in 0..num_teams {
        entries.push(ComboEntry {
            label: lookup_game_text(&format!("Team:{}", team + 1)),
            data: team as i32,
            color: random_color,
        });
    }
    entries
}

/// C++ `PopulateStartingCashComboBox`.
pub fn populate_starting_cash_combo_box(
    game: &GameInfo,
    cash_choices: &[u32],
) -> (Vec<ComboEntry>, usize) {
    let current = game.get_starting_cash().count_money();
    let format = lookup_game_text("GUI:StartingMoneyFormat");
    let mut entries = Vec::new();
    let mut selected = 0usize;
    for (idx, amount) in cash_choices.iter().enumerate() {
        let label = if format.contains("%d") {
            format.replace("%d", &amount.to_string())
        } else {
            format!("{format} {amount}")
        };
        if *amount == current {
            selected = idx;
        }
        entries.push(ComboEntry {
            label,
            data: *amount as i32,
            color: 0x00FFFFFF,
        });
    }
    if entries.is_empty() {
        entries.push(ComboEntry {
            label: current.to_string(),
            data: current as i32,
            color: 0x00FFFFFF,
        });
    }
    (entries, selected)
}

/// C++ `EnableAcceptControls` for one slot (`slot_num`) or every slot (`None`).
pub fn enable_accept_controls(
    enabled: bool,
    game: &GameInfo,
    slot_num: Option<usize>,
    local_slot: Option<usize>,
    is_host: bool,
) -> Vec<(usize, SlotControlEnablement)> {
    let range: Box<dyn Iterator<Item = usize>> = match slot_num {
        Some(slot) => Box::new(std::iter::once(slot)),
        None => Box::new(0..MAX_SLOTS),
    };
    range
        .filter_map(|idx| {
            let slot = game.get_slot(idx)?;
            let occupied = slot.is_occupied();
            let observer = slot.get_player_template() == PLAYERTEMPLATE_OBSERVER;
            let local = local_slot == Some(idx);
            let can_edit = enabled && occupied && (is_host || local);
            Some((
                idx,
                SlotControlEnablement {
                    player_combo: enabled && (is_host || local),
                    color_combo: can_edit && !observer,
                    template_combo: can_edit,
                    team_combo: can_edit && !observer,
                    accept_button: enabled && occupied && !is_host,
                    start_position: can_edit && !observer,
                },
            ))
        })
        .collect()
}

/// C++ `UpdateSlotList` data (menus apply this to combo/accept gadgets).
pub fn update_slot_list(
    game: &GameInfo,
    local_slot: Option<usize>,
    is_host: bool,
) -> Vec<SlotListRow> {
    if !are_slot_list_updates_enabled() {
        return Vec::new();
    }
    let enablement = enable_accept_controls(true, game, None, local_slot, is_host);
    (0..MAX_SLOTS)
        .filter_map(|idx| {
            let slot = game.get_slot(idx)?;
            let controls = enablement
                .iter()
                .find(|(i, _)| *i == idx)
                .map(|(_, e)| e.clone())
                .unwrap_or_default();
            Some(SlotListRow {
                slot_index: idx,
                occupied: slot.is_occupied(),
                is_player: slot.is_human(),
                is_ai: slot.is_ai(),
                is_local: local_slot == Some(idx),
                is_observer: slot.get_player_template() == PLAYERTEMPLATE_OBSERVER,
                accepted: slot.is_accepted(),
                name: slot.get_name().to_string(),
                color: slot.get_color(),
                player_template: slot.get_player_template(),
                team: slot.get_team_number(),
                start_pos: slot.get_start_pos(),
                enablement: controls,
            })
        })
        .collect()
}

/// C++ `ShowUnderlyingGUIElements` — names of gadgets a layout should hide/show.
pub fn underlying_gui_elements(
    gadgets_to_hide: &[&str],
    per_player_gadgets_to_hide: &[&str],
) -> (Vec<String>, Vec<String>) {
    let global = gadgets_to_hide.iter().map(|s| (*s).to_string()).collect();
    let mut per_player = Vec::new();
    for gadget in per_player_gadgets_to_hide {
        for slot in 0..MAX_SLOTS {
            per_player.push(format!("{gadget}{slot}"));
        }
    }
    (global, per_player)
}

/// Convenience starting-cash list used when MultiplayerSettings is unavailable.
pub fn default_starting_cash_choices() -> Vec<Money> {
    [5000, 7500, 10000, 15000, 20000]
        .into_iter()
        .map(Money::new)
        .collect()
}
