//! Control-bar multi-select helpers.
//!
//! Ported from `ControlBarMultiSelect.cpp`.

use super::control_bar::ControlBar;
use super::{CommandOption, ControlBarContext};
use gamelogic::command_button::MAX_COMMANDS_PER_SET;
use gamelogic::commands::CommandType;
use gamelogic::common::types::{KindOf, OBJECT_STATUS_SOLD};
use gamelogic::object::registry::OBJECT_REGISTRY;

/// Populate multi-select buttons by intersecting command-set names
/// (host/presentation path — no OBJECT_REGISTRY).
pub(super) fn populate_multi_select_commands_from_sets(
    bar: &ControlBar,
    context: &mut ControlBarContext,
    command_set_names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if command_set_names.len() < 2 {
        return Ok(());
    }
    let Some(control_bar) = bar.control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = bar.ini_buttons() else {
        return Ok(());
    };

    let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
        vec![None; MAX_COMMANDS_PER_SET];
    let mut saw_first = false;

    for name in command_set_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let command_set = control_bar
            .find_command_set_by_name(name)
            .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
        let Some(command_set) = command_set else {
            // C++ clears the shared set when a selected object has no command set.
            common_slots.fill(None);
            saw_first = true;
            break;
        };
        intersect_command_set_into_slots(&mut common_slots, &mut saw_first, &command_set);
    }

    if !saw_first {
        return Ok(());
    }
    push_common_slots(bar, context, &common_bar, common_slots);
    Ok(())
}

/// Populate command buttons shared across all selected objects via OBJECT_REGISTRY.
///
/// Matches the original C++ behaviour:
/// - starts with commands from the first valid selected object
/// - removes slots that diverge on subsequent objects
/// - keeps `ATTACK_MOVE` if any selected unit contributes it in that slot
pub(super) fn populate_multi_select_commands(
    bar: &ControlBar,
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(control_bar) = bar.control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = bar.ini_buttons() else {
        return Ok(());
    };

    let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
        vec![None; MAX_COMMANDS_PER_SET];
    let mut saw_first_drawable = false;

    for object_id in &context.selected_objects {
        let Some(object_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
            continue;
        };

        let Ok(object) = object_arc.read() else {
            continue;
        };

        if object.is_kind_of(KindOf::IgnoredInGui) || object.test_status(OBJECT_STATUS_SOLD) {
            continue;
        }

        let command_set_name = object.get_command_set_string().to_string();
        let command_set = control_bar
            .find_command_set_by_name(&command_set_name)
            .or_else(|| {
                control_bar.find_command_set_by_name(&command_set_name.to_ascii_uppercase())
            });

        let Some(command_set) = command_set else {
            // C++ clears the shared set when a selected object has no command set.
            common_slots.fill(None);
            saw_first_drawable = true;
            break;
        };

        intersect_command_set_into_slots(&mut common_slots, &mut saw_first_drawable, &command_set);
    }

    if !saw_first_drawable {
        return Ok(());
    }
    push_common_slots(bar, context, &common_bar, common_slots);
    Ok(())
}

fn intersect_command_set_into_slots(
    common_slots: &mut [Option<gamelogic::command_button::CommandButton>],
    saw_first: &mut bool,
    command_set: &gamelogic::command_button::CommandSet,
) {
    if !*saw_first {
        for slot in 0..MAX_COMMANDS_PER_SET {
            let Some(button) = command_set
                .buttons
                .get(slot)
                .and_then(|button| button.as_ref())
            else {
                continue;
            };

            if (button.get_options_bits() & CommandOption::OkForMultiSelect as u32) != 0 {
                common_slots[slot] = Some(button.clone());
            }
        }
        *saw_first = true;
        return;
    }

    for slot in 0..MAX_COMMANDS_PER_SET {
        let command = command_set
            .buttons
            .get(slot)
            .and_then(|button| button.as_ref());
        let common = common_slots[slot].as_ref();

        let attack_move = command
            .map(|button| button.get_command_type() == CommandType::DoAttackMoveTo)
            .unwrap_or(false)
            || common
                .map(|button| button.get_command_type() == CommandType::DoAttackMoveTo)
                .unwrap_or(false);

        if attack_move && common_slots[slot].is_none() {
            common_slots[slot] = command.cloned();
            continue;
        }

        if attack_move {
            continue;
        }

        let matches = match (command, common) {
            (Some(a), Some(b)) => a.get_id() == b.get_id(),
            (None, None) => true,
            _ => false,
        };

        if !matches {
            common_slots[slot] = None;
        }
    }
}

fn push_common_slots(
    bar: &ControlBar,
    context: &mut ControlBarContext,
    common_bar: &game_engine::common::ini::ini_command_button::ControlBar,
    common_slots: Vec<Option<gamelogic::command_button::CommandButton>>,
) {
    context.available_commands.clear();
    for slot in 0..14 {
        let button = common_slots.get(slot).and_then(|b| b.as_ref());
        context
            .available_commands
            .push(bar.command_from_set_slot(common_bar, button));
    }
}

/// Residual: last multi-select action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ResidualMultiSelectAction {
    #[default]
    None = 0,
    Populate = 1,
    Clear = 2,
    Intersect = 3,
    AttackMoveKeep = 4,
}

/// Residual: max command slots per set (CommandSet.cpp).
pub const MULTI_SELECT_MAX_COMMANDS_PER_SET: usize = 18;

/// Residual: OK_FOR_MULTI_SELECT option bit (CommandOption).
pub const MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT: u32 = 0x00000100;

/// Per-ControlBar multi-select presentation latch (was process atomics).
///
/// C++ ControlBarMultiSelect keeps intersection state on the bar; Rust owns it
/// per client so multiple instances never share process statics.
#[derive(Debug, Clone, Default)]
pub struct MultiSelectLatch {
    pub last_action: ResidualMultiSelectAction,
    pub selected_count: usize,
    pub actionable_count: usize,
    pub common_command_count: usize,
    pub portrait_agrees: bool,
    pub attack_move_kept: bool,
}

impl MultiSelectLatch {
    pub fn clear(&mut self) {
        *self = Self::default();
        self.last_action = ResidualMultiSelectAction::Clear;
    }

    pub fn record_populate(
        &mut self,
        selected_count: usize,
        actionable_count: usize,
        common_command_count: usize,
        portrait_agrees: bool,
        attack_move_kept: bool,
    ) {
        self.selected_count = selected_count;
        self.actionable_count = actionable_count;
        self.common_command_count = common_command_count;
        self.portrait_agrees = portrait_agrees;
        self.attack_move_kept = attack_move_kept;
        self.last_action = if attack_move_kept {
            ResidualMultiSelectAction::AttackMoveKeep
        } else {
            ResidualMultiSelectAction::Populate
        };
    }
}

/// Residual: last multi-select residual action.
pub fn residual_multi_select_last_action(latch: &MultiSelectLatch) -> ResidualMultiSelectAction {
    latch.last_action
}

/// Residual: selected unit count latch.
pub fn residual_multi_select_selected_count(latch: &MultiSelectLatch) -> usize {
    latch.selected_count
}

/// Residual: actionable unit count latch (not sold/ignored).
pub fn residual_multi_select_actionable_count(latch: &MultiSelectLatch) -> usize {
    latch.actionable_count
}

/// Residual: common command slot count latch (non-empty after intersect).
pub fn residual_multi_select_common_command_count(latch: &MultiSelectLatch) -> usize {
    latch.common_command_count
}

/// Residual: portrait agreement latch.
pub fn residual_multi_select_portrait_agrees(latch: &MultiSelectLatch) -> bool {
    latch.portrait_agrees
}

/// Residual: attack-move kept latch.
pub fn residual_multi_select_attack_move_kept(latch: &MultiSelectLatch) -> bool {
    latch.attack_move_kept
}

/// Residual unit command set for multi-select intersection peels.
#[derive(Debug, Clone)]
pub struct ResidualMultiSelectUnit {
    pub portrait_name: String,
    pub ignored_in_gui: bool,
    pub sold: bool,
    /// Per-slot command names; empty string = empty slot.
    pub command_names: Vec<String>,
    /// Per-slot option bits (includes OK_FOR_MULTI_SELECT).
    pub option_bits: Vec<u32>,
}

impl ResidualMultiSelectUnit {
    /// Build residual unit with named multi-select-ok commands.
    pub fn with_multi_ok(portrait: &str, commands: &[&str]) -> Self {
        Self {
            portrait_name: portrait.to_string(),
            ignored_in_gui: false,
            sold: false,
            command_names: commands.iter().map(|s| (*s).to_string()).collect(),
            option_bits: commands
                .iter()
                .map(|_| MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT)
                .collect(),
        }
    }
}

/// Residual: clear multi-select residual state.
pub fn simulate_multi_select_clear(latch: &mut MultiSelectLatch) -> bool {
    latch.clear();
    residual_multi_select_selected_count(latch) == 0
        && residual_multi_select_common_command_count(latch) == 0
}

/// Residual: populate multi-select by intersecting OK_FOR_MULTI_SELECT slots.
///
/// Mirrors C++ ControlBarMultiSelect:
/// - ignore sold/ignored units
/// - first unit seeds OK_FOR_MULTI_SELECT slots
/// - subsequent units clear divergent slots
/// - AttackMove is kept if any unit contributes it in that slot
pub fn simulate_multi_select_populate(
    latch: &mut MultiSelectLatch,
    units: &[ResidualMultiSelectUnit],
) -> bool {
    if units.len() < 2 {
        return false;
    }
    let slot_count = units
        .iter()
        .map(|u| u.command_names.len().max(u.option_bits.len()))
        .max()
        .unwrap_or(0)
        .min(MULTI_SELECT_MAX_COMMANDS_PER_SET);
    let mut common: Vec<Option<String>> = vec![None; slot_count];
    let mut portrait: Option<String> = None;
    let mut first = true;
    let mut actionable = 0usize;
    let mut attack_move_kept = false;

    for unit in units.iter().filter(|u| !u.ignored_in_gui && !u.sold) {
        actionable += 1;
        if first {
            for i in 0..slot_count {
                let name = unit.command_names.get(i).map(|s| s.as_str()).unwrap_or("");
                let bits = unit.option_bits.get(i).copied().unwrap_or(0);
                if !name.is_empty() && (bits & MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT) != 0 {
                    common[i] = Some(name.to_string());
                    if name.eq_ignore_ascii_case("Command_AttackMove")
                        || name.eq_ignore_ascii_case("AttackMove")
                    {
                        attack_move_kept = true;
                    }
                }
            }
            portrait = Some(unit.portrait_name.clone());
            first = false;
            continue;
        }
        if portrait.as_deref() != Some(unit.portrait_name.as_str()) {
            portrait = None;
        }
        for i in 0..slot_count {
            let name = unit.command_names.get(i).map(|s| s.as_str()).unwrap_or("");
            let bits = unit.option_bits.get(i).copied().unwrap_or(0);
            let existing = common[i].clone();
            let unit_am = name.eq_ignore_ascii_case("Command_AttackMove")
                || name.eq_ignore_ascii_case("AttackMove");
            let exist_am = existing
                .as_deref()
                .map(|n| {
                    n.eq_ignore_ascii_case("Command_AttackMove")
                        || n.eq_ignore_ascii_case("AttackMove")
                })
                .unwrap_or(false);
            if exist_am || unit_am {
                // C++ keeps AttackMove if any selected unit contributes it.
                if exist_am || ((bits & MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT) != 0 && unit_am) {
                    if common[i].is_none() && unit_am {
                        common[i] = Some(name.to_string());
                    }
                    attack_move_kept = true;
                    continue;
                }
            }
            match (&existing, name.is_empty()) {
                (Some(ex), false)
                    if ex == name && (bits & MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT) != 0 => {}
                (Some(_), _) => common[i] = None,
                (None, _) => {}
            }
        }
    }

    if first || actionable < 2 {
        return false;
    }

    let common_count = common.iter().filter(|c| c.is_some()).count();
    latch.record_populate(
        units.len(),
        actionable,
        common_count,
        portrait.is_some(),
        attack_move_kept,
    );
    residual_multi_select_actionable_count(latch) >= 2
}

/// Residual: two-unit same-command composite (portrait agrees).
pub fn simulate_multi_select_prepare_same_commands(latch: &mut MultiSelectLatch) -> bool {
    let a = ResidualMultiSelectUnit::with_multi_ok(
        "Ranger",
        &["Command_Guard", "Command_AttackMove", "Command_Stop"],
    );
    let b = ResidualMultiSelectUnit::with_multi_ok(
        "Ranger",
        &["Command_Guard", "Command_AttackMove", "Command_Stop"],
    );
    simulate_multi_select_populate(latch, &[a, b])
        && residual_multi_select_common_command_count(latch) == 3
        && residual_multi_select_portrait_agrees(latch)
        && residual_multi_select_attack_move_kept(latch)
}

/// Residual: divergent commands clear non-AttackMove slots.
pub fn simulate_multi_select_prepare_divergent(latch: &mut MultiSelectLatch) -> bool {
    let a =
        ResidualMultiSelectUnit::with_multi_ok("Ranger", &["Command_Guard", "Command_AttackMove"]);
    let b = ResidualMultiSelectUnit::with_multi_ok(
        "MissileDefender",
        &["Command_Stop", "Command_AttackMove"],
    );
    if !simulate_multi_select_populate(latch, &[a, b]) {
        return false;
    }
    latch.last_action = ResidualMultiSelectAction::Intersect;
    // Guard/Stop diverge → cleared; AttackMove kept.
    residual_multi_select_common_command_count(latch) == 1
        && residual_multi_select_attack_move_kept(latch)
        && !residual_multi_select_portrait_agrees(latch)
}
