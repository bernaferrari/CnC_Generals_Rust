use std::collections::HashMap;
use std::sync::RwLock;

use crate::command_button::{CommandButton, CommandButtonId, CommandSet, MAX_COMMANDS_PER_SET};
use crate::upgrade::center::UpgradeCenter;
use crate::commands::command::CommandType;
use crate::common::KindOf;
use crate::common::types::ControlBarInterface;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{CommandSetHandle, PlayerHandle, ThingTemplateHandle};
use game_engine::rts::academy_stats::{
    AcademyTemplateContext, set_academy_template_context_provider,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ControlBarBridge {
    buttons_by_id: HashMap<CommandButtonId, CommandButton>,
    command_sets: HashMap<String, CommandSet>,
    /// The owning world's `GameLogic::m_controlBarOverrides` (bound when the
    /// bridge is installed in that world's slot); unbound bridges have none.
    overrides: Option<ControlBarOverrides>,
}

/// C++ `GameLogic::m_controlBarOverrides`, shared by a world's GameLogic and
/// the CommandSets of that world's ControlBar (key: `'0'+slot` + set name).
pub type ControlBarOverrides = Arc<RwLock<HashMap<String, Option<String>>>>;

/// C++ `GameLogic` override key: `'0'+slot` followed by the CommandSet name.
pub(crate) fn control_bar_override_key(command_set_name: &str, slot: i32) -> Option<String> {
    if !(0..MAX_COMMANDS_PER_SET as i32).contains(&slot) {
        return None;
    }
    let slot_prefix = char::from_u32('0' as u32 + slot as u32)?;
    Some(format!("{}{}", slot_prefix, command_set_name))
}

/// Result of resolving a `UNIT_BUILD` authorization through the live parsed
/// CommandButton/CommandSet/Object catalog.
///
/// `Unavailable` deliberately means that the shared catalog has not supplied
/// an identity for this producer yet.  Callers may use their legacy fallback
/// in that one case.  Once a producer resolves to a parsed CommandSet,
/// `Rejected` is fail-closed: no inferred factory family or button-name
/// convention is consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedUnitBuildAuthorization {
    Unavailable,
    Rejected,
    Authorized,
}

/// Match the typed GameLogic operation and exact Object template identity of
/// one parsed CommandButton.
///
/// The C++ GUI uses `UNIT_BUILD` as the operation and `Object = <template>`
/// as the target.  Keeping that pair separate prevents a similarly named
/// button, or a prefix/suffix of a template name, from granting production.
fn is_exact_unit_build_identity(
    command_type: CommandType,
    object_template: Option<&str>,
    requested_template: &str,
) -> bool {
    command_type == CommandType::QueueUnitCreate && object_template == Some(requested_template)
}

impl ControlBarBridge {
    /// Test bridge holding exactly `command_sets`, keyed by their names.
    #[cfg(test)]
    pub(crate) fn from_command_sets(command_sets: Vec<CommandSet>) -> Self {
        Self {
            buttons_by_id: HashMap::new(),
            command_sets: command_sets.into_iter().map(|set| (set.name.clone(), set)).collect(),
            overrides: None,
        }
    }

    /// Bind this catalog to its world's GameLogic override table (C++
    /// `CommandSet::getCommandButton` consults that world's TheGameLogic).
    pub(crate) fn bind_overrides(&mut self, overrides: ControlBarOverrides) {
        for set in self.command_sets.values_mut() {
            set.overrides = Some(Arc::clone(&overrides));
        }
        self.overrides = Some(overrides);
    }

    /// Build from the CommandButton/CommandSet catalog owned by `common`
    /// (C++ ControlBar.cpp:1060-1120: the engine's own ControlBar parses them).
    pub fn build_from_common(
        common: &game_engine::common::common_services::CommonServices,
    ) -> Result<Self, String> {
        Self::build_from_common_using(common, CommandButton::from_common)
    }

    pub fn build_from_common_with_upgrade_definitions(
        common: &game_engine::common::common_services::CommonServices,
        upgrade_definitions: &UpgradeCenter,
    ) -> Result<Self, String> {
        Self::build_from_common_using(common, |id, button| {
            CommandButton::from_common_with_upgrade_definitions(id, button, upgrade_definitions)
        })
    }

    fn build_from_common_using(
        common: &game_engine::common::common_services::CommonServices,
        mut make_button: impl FnMut(
            CommandButtonId,
            &game_engine::common::ini::ini_command_button::CommandButton,
        ) -> CommandButton,
    ) -> Result<Self, String> {
        let control_bar = common.ini.control_bar.read();
        let mut button_names: Vec<String> = control_bar
            .get_button_names()
            .into_iter()
            .cloned()
            .collect();
        button_names.sort();

        let mut buttons_by_id = HashMap::new();
        let mut name_to_id = HashMap::new();
        let mut next_id: CommandButtonId = 1;

        for name in button_names {
            let Some(common_button) = control_bar.find_command_button_resolved(&name) else {
                continue;
            };
            let command_button = make_button(next_id, common_button);
            name_to_id.insert(name.clone(), next_id);
            buttons_by_id.insert(next_id, command_button);
            next_id += 1;
        }

        let mut command_sets = HashMap::new();
        {
            let manager = common.ini.command_set.read();
            for (name, set) in manager.iter_resolved_sets() {
                let mut command_set = CommandSet::new(name.clone());

                for index in 0..MAX_COMMANDS_PER_SET {
                    if let Some(button_name) = set.get_button_at_position(index) {
                        if let Some(id) = name_to_id.get(button_name) {
                            if let Some(button) = buttons_by_id.get(id) {
                                command_set.set_command_button(index, Some(button.clone()));
                            }
                        }
                    }
                }

                command_sets.insert(name.clone(), command_set);
            }
        }

        Ok(Self {
            buttons_by_id,
            command_sets,
            overrides: None,
        })
    }

    pub fn find_command_button_by_name(&self, name: &str) -> Option<&CommandButton> {
        self.buttons_by_id
            .values()
            .find(|button| button.name.eq_ignore_ascii_case(name))
    }

    pub fn find_command_set_by_name(&self, name: &str) -> Option<&CommandSet> {
        self.command_sets.get(name)
    }

    /// Whether this bridge was constructed from a non-empty parsed retail
    /// CommandButton + CommandSet catalog.
    fn has_resolved_command_set_catalog(&self) -> bool {
        !self.buttons_by_id.is_empty() && !self.command_sets.is_empty()
    }

    /// Check one producer CommandSet against an exact requested Object
    /// template.  `None` means the exact CommandSet identity is absent; it
    /// does not perform a case fold or an alias lookup.
    fn exact_unit_build_authorization(
        &self,
        command_set_name: &str,
        requested_template: &str,
    ) -> Option<bool> {
        let command_set = self.command_sets.get(command_set_name)?;
        Some(command_set.buttons.iter().flatten().any(|button| {
            is_exact_unit_build_identity(
                button.get_command_type(),
                button.get_object_template_name(),
                requested_template,
            )
        }))
    }

    pub fn set_command_set_slot_override(
        &mut self,
        command_set_name: &str,
        slot: usize,
        button_name: Option<&str>,
    ) -> Result<(), String> {
        if slot >= MAX_COMMANDS_PER_SET {
            return Err(format!(
                "Command slot {} out of range [0, {})",
                slot, MAX_COMMANDS_PER_SET
            ));
        }

        let set_key = self
            .command_sets
            .keys()
            .find(|name| name.eq_ignore_ascii_case(command_set_name))
            .cloned()
            .ok_or_else(|| format!("Command set '{}' not found", command_set_name))?;

        let button = if let Some(name) = button_name {
            Some(
                self.find_command_button_by_name(name)
                    .cloned()
                    .ok_or_else(|| format!("Command button '{}' not found", name))?,
            )
        } else {
            None
        };

        let Some(command_set) = self.command_sets.get_mut(&set_key) else {
            return Err(format!("Command set '{}' not found", command_set_name));
        };
        command_set.buttons[slot] = button;
        Ok(())
    }
}

fn with_bridge_slot<R>(f: impl FnOnce(&RwLock<Option<Arc<ControlBarBridge>>>) -> R) -> R {
    let services = crate::system::engine_stores::active_services();
    f(services.control_bar_bridge())
}

pub fn initialize_control_bar_bridge_from_common() -> Result<(), String> {
    refresh_control_bar_bridge_from_common()
}

/// Rebuild the published world's bridge from that same world's catalog.
pub fn refresh_control_bar_bridge_from_common() -> Result<(), String> {
    refresh_control_bar_bridge_for_world(&crate::system::engine_stores::active_services(), None)
}

/// Refresh from explicit receiving-session upgrade content without selecting Core AI.
pub fn refresh_control_bar_bridge_from_common_with_upgrade_definitions(
    upgrade_definitions: &UpgradeCenter,
) -> Result<(), String> {
    refresh_control_bar_bridge_for_world(
        &crate::system::engine_stores::active_services(),
        Some(upgrade_definitions),
    )
}

/// Rebuild `world`'s bridge from the CommandButton/CommandSet catalog owned by
/// `world.common()`. Neither the catalog nor the slot is read from a peer.
pub fn refresh_control_bar_bridge_for_world(
    world: &crate::system::engine_stores::WorldServices,
    upgrade_definitions: Option<&UpgradeCenter>,
) -> Result<(), String> {
    let bridge = match upgrade_definitions {
        Some(upgrades) => {
            ControlBarBridge::build_from_common_with_upgrade_definitions(world.common(), upgrades)?
        }
        None => ControlBarBridge::build_from_common(world.common())?,
    };
    let mut bridge = bridge;
    bridge.bind_overrides(Arc::clone(world.control_bar_overrides()));
    let mut guard = world
        .control_bar_bridge()
        .write()
        .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
    *guard = Some(Arc::new(bridge));
    Ok(())
}

/// `world`'s own resolved ControlBar catalog, read from that world's slot.
pub fn control_bar_bridge_for_world(
    world: &crate::system::engine_stores::WorldServices,
) -> Option<Arc<ControlBarBridge>> {
    world.control_bar_bridge().read().ok().and_then(|guard| guard.clone())
}

fn replace_control_bar_bridge(bridge: ControlBarBridge) -> Result<(), String> {
    with_bridge_slot(|slot| {
        let mut guard = slot
            .write()
            .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
        *guard = Some(Arc::new(bridge));
        Ok(())
    })
}

/// C++ `TheControlBar` of the GameEngine that owns `object`: the ControlBar of
/// the world it was assembled in. An Object of no world has no ControlBar.
pub(crate) fn control_bar_bridge_for(
    object: &crate::object::Object,
) -> Option<Arc<ControlBarBridge>> {
    let world = object.owning_world_services()?;
    control_bar_bridge_in(&world)
}

/// `world`'s ControlBar (C++ `TheControlBar` of that GameEngine).
pub fn control_bar_bridge_in(
    world: &crate::system::engine_stores::WorldServices,
) -> Option<Arc<ControlBarBridge>> {
    world.control_bar_bridge().read().ok().and_then(|slot| slot.clone())
}


/// Authorize a producer/template pair from the parsed retail CommandSet
/// catalog used by GameClient's ControlBar.
///
/// The lookup boundary is intentionally narrow and exact:
/// `ThingTemplate::<producer>.CommandSet` -> resolved CommandSet -> typed
/// `QueueUnitCreate` button -> `Object = <producible>`.  Missing catalog data
/// is reported separately so Main can retain its compatibility fallback only
/// while startup data is unavailable.
pub fn parsed_unit_build_authorization(
    world: &crate::system::engine_stores::WorldServices,
    producer_template: &str,
    producible_template: &str,
) -> ParsedUnitBuildAuthorization {
    if producer_template.is_empty() || producible_template.is_empty() {
        return ParsedUnitBuildAuthorization::Rejected;
    }

    // Do not use `TheThingFactory::find_template` here: that convenience API
    // may initialize a fallback factory on a miss.  Production authorization
    // must only read the currently loaded, exact retail template identity.
    let command_set_name = {
        let Ok(factory_guard) = game_engine::common::thing::get_thing_factory() else {
            return ParsedUnitBuildAuthorization::Unavailable;
        };
        let Some(factory) = factory_guard.as_ref() else {
            return ParsedUnitBuildAuthorization::Unavailable;
        };
        if factory.first_template().is_none() {
            return ParsedUnitBuildAuthorization::Unavailable;
        }
        let Some(producer) = factory.find_template(producer_template, false) else {
            // The shared source has no exact identity for this producer (for
            // example a test-only template), so it cannot supersede Main's
            // compatibility path.
            return ParsedUnitBuildAuthorization::Unavailable;
        };
        let name = producer.get_command_set_string().as_str();
        if name.is_empty() {
            return ParsedUnitBuildAuthorization::Rejected;
        }
        name.to_string()
    };

    // The ControlBar of the world authorizing the build.
    let Some(bridge) = world.control_bar_bridge().read().ok().and_then(|slot| slot.clone()) else {
        return ParsedUnitBuildAuthorization::Unavailable;
    };
    if !bridge.has_resolved_command_set_catalog() {
        return ParsedUnitBuildAuthorization::Unavailable;
    }

    match bridge.exact_unit_build_authorization(&command_set_name, producible_template) {
        Some(true) => ParsedUnitBuildAuthorization::Authorized,
        // Once the producer supplied an exact CommandSet identity, a missing
        // set or target is an explicit denial, never a guessed factory match.
        Some(false) | None => ParsedUnitBuildAuthorization::Rejected,
    }
}

/// Insert a synthetic command button + command-set slot for tests / internal harnesses.
/// C++ scripts look these up via `TheControlBar->findCommandButton` / `findCommandSet`.
#[cfg(any(test, feature = "internal"))]
pub fn install_test_command_button(
    button: CommandButton,
    command_set_name: &str,
    slot: usize,
) -> Result<(), String> {
    if slot >= MAX_COMMANDS_PER_SET {
        return Err(format!(
            "Command slot {} out of range [0, {})",
            slot, MAX_COMMANDS_PER_SET
        ));
    }

    with_bridge_slot(|bridge_slot| {
        let mut guard = bridge_slot
            .write()
            .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
        let bridge = guard.get_or_insert_with(|| {
            Arc::new(ControlBarBridge {
                buttons_by_id: HashMap::new(),
                command_sets: HashMap::new(),
                overrides: None,
            })
        });
        let bridge = Arc::make_mut(bridge);
        let id = button.get_id();
        bridge.buttons_by_id.insert(id, button.clone());
        let overrides = bridge.overrides.clone();
        let set = bridge
            .command_sets
            .entry(command_set_name.to_string())
            .or_insert_with(|| CommandSet::new(command_set_name.to_string()).with_overrides(overrides));
        if !set.set_command_button(slot, Some(button)) {
            return Err(format!("Failed to set command button at slot {}", slot));
        }
        Ok(())
    })
}

pub fn register_academy_template_context_provider() {
    set_academy_template_context_provider(|player| find_academy_template_context(player));
}

fn find_academy_template_context(player: PlayerHandle) -> Option<AcademyTemplateContext> {
    if !player.is_valid() || player.value() > i32::MAX as u32 {
        return None;
    }

    let player_index = player.value() as i32;
    let __player_list_arc = player_list();
        let player_arc = __player_list_arc.read()
        .ok()
        .and_then(|players| players.get_player(player_index).cloned())?;
    let object_ids = get_object_manager()
        .read()
        .ok()?
        .get_objects_owned_by_player(player.value());
    let object_manager = get_object_manager();
    let object_manager = object_manager.read().ok()?;

    for object_id in object_ids {
        let Some(object_arc) = object_manager.get_object(object_id) else {
            continue;
        };
        // C++ TheControlBar of the GameEngine that owns the object.
        let Some((command_set_name, Some(control_bar))) = object_arc
            .read()
            .ok()
            .and_then(|object| {
                object.base().read().ok().map(|base| {
                    (base.get_command_set_string().to_string(), control_bar_bridge_for(&base))
                })
            })
        else {
            continue;
        };
        if command_set_name.is_empty() {
            continue;
        }

        let Some(command_set) = control_bar.find_command_set_by_name(&command_set_name) else {
            continue;
        };
        let mut context = AcademyTemplateContext {
            dozer_command_set: CommandSetHandle::new(NameKeyGenerator::name_to_key(
                command_set.name.as_str(),
            )),
            ..AcademyTemplateContext::default()
        };

        for index in 0..MAX_COMMANDS_PER_SET {
            let Some(button) = command_set.get_command_button(index) else {
                continue;
            };
            let Some(template) = button.get_thing_template() else {
                continue;
            };

            if template.is_kind_of(KindOf::CommandCenter) {
                context.command_center_template = ThingTemplateHandle::new(template.get_id());
            } else if template.is_kind_of(KindOf::FSSupplyCenter) {
                context.supply_center_template = ThingTemplateHandle::new(template.get_id());
                context.supply_center_cost = player_arc
                    .read()
                    .ok()
                    .map(|player| template.calc_cost_to_build(Some(&*player)).max(0) as u32)
                    .unwrap_or_else(|| template.get_build_cost().max(0) as u32);
            }
        }

        if context.command_center_template.is_valid() || context.supply_center_template.is_valid() {
            return Some(context);
        }
    }

    None
}

/// C++ `doRemove/AddCommandBarButton`: the dispatching world's ControlBar
/// slot and its `GameLogic::setControlBarOverride` table (shared with that
/// ControlBar), never the world published on the thread.
pub fn set_command_set_slot_override_in(
    bridge_slot: &RwLock<Option<Arc<ControlBarBridge>>>,
    command_set_name: &str,
    slot: usize,
    button_name: Option<&str>,
) -> Result<(), String> {
    let mut guard = bridge_slot
        .write()
        .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
    let Some(bridge) = guard.as_mut() else {
        return Err("ControlBarBridge not initialized".to_string());
    };
    let bridge = Arc::make_mut(bridge);
    bridge.set_command_set_slot_override(command_set_name, slot, button_name)?;
    if let (Some(overrides), Some(key)) = (
        bridge.overrides.as_ref(),
        control_bar_override_key(command_set_name, slot as i32),
    ) {
        overrides
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, button_name.map(str::to_string));
    }
    Ok(())
}

/// Notify `world`'s ControlBar UI that its state needs a refresh.
pub fn mark_ui_dirty_in(
    hooks: &std::sync::Mutex<Option<Arc<dyn ControlBarUiHooks>>>,
) {
    let hooks = hooks.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(hooks) = hooks {
        hooks.mark_ui_dirty();
    }
}

/// Hooks for notifying the live UI to refresh control bar state.
pub trait ControlBarUiHooks: Send + Sync {
    fn mark_ui_dirty(&self);
    fn on_player_science_purchase_points_changed(&self, player_id: i32, points: i32);
    fn on_player_rank_changed(&self, player_id: i32, rank_level: i32, points: i32);
}

pub fn register_control_bar_ui_hooks(hooks: Arc<dyn ControlBarUiHooks>) -> bool {
    let services = crate::system::engine_stores::active_services();
    let mut slot = services
        .control_bar_ui_hooks()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *slot = Some(hooks);
    true
}


/// C++ `TheControlBar->markUIDirty()` from code running for `object`: the
/// ControlBar of the world it was assembled in.
pub(crate) fn mark_ui_dirty_for(object: &crate::object::Object) {
    mark_ui_dirty_from(object.owning_world_services().as_ref());
}

/// `world`'s ControlBar; without an owning world there is nothing to mark.
pub(crate) fn mark_ui_dirty_from(
    world: Option<&Arc<crate::system::engine_stores::WorldServices>>,
) {
    if let Some(world) = world {
        mark_ui_dirty_in(world.control_bar_ui_hooks());
    }
}

/// C++ `TheControlBar->markUIDirty()` on `world`'s ControlBar.
pub fn mark_world_ui_dirty(world: &crate::system::engine_stores::WorldServices) {
    mark_ui_dirty_in(world.control_bar_ui_hooks());
}



impl ControlBarInterface for ControlBarBridge {
    fn find_command_set(&self, name: &str) -> Option<&dyn std::any::Any> {
        self.command_sets
            .get(name)
            .map(|set| set as &dyn std::any::Any)
    }

    fn get_command_button(&self, button_id: CommandButtonId) -> Option<&CommandButton> {
        self.buttons_by_id.get(&button_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_unit_build_identity_is_typed_and_exact() {
        // `AmericaBarracksCommandSet` references this retail button, whose
        // CommandButton.ini definition is `Command = UNIT_BUILD` and
        // `Object = AmericaInfantryRanger`.
        assert!(is_exact_unit_build_identity(
            CommandType::QueueUnitCreate,
            Some("AmericaInfantryRanger"),
            "AmericaInfantryRanger",
        ));

        // Neither a near template name nor a different parsed operation may
        // authorize the request.
        assert!(!is_exact_unit_build_identity(
            CommandType::QueueUnitCreate,
            Some("AmericaInfantryRanger"),
            "AmericaInfantryRangerPrototype",
        ));
        assert!(!is_exact_unit_build_identity(
            CommandType::QueueUpgrade,
            Some("AmericaInfantryRanger"),
            "AmericaInfantryRanger",
        ));
    }

    #[test]
    fn retail_unit_build_button_keeps_the_parsed_object_identity() {
        use game_engine::common::ini::ini_command_button::CommandButton as ParsedCommandButton;

        // These are the retail CommandButton.ini identities used by
        // AmericaBarracksCommandSet.  Build the GameLogic bridge shape from
        // the parsed operation/object pair, rather than a button-name rule.
        let mut parsed =
            ParsedCommandButton::new("Command_ConstructAmericaInfantryRanger".to_string());
        parsed.command = "UNIT_BUILD".to_string();
        parsed.object = "AmericaInfantryRanger".to_string();
        let ranger = CommandButton::from_common(1, &parsed);

        assert_eq!(ranger.get_command_type(), CommandType::QueueUnitCreate);
        assert_eq!(
            ranger.get_object_template_name(),
            Some("AmericaInfantryRanger")
        );

        let mut command_set = CommandSet::new("AmericaBarracksCommandSet".to_string());
        assert!(command_set.set_command_button(0, Some(ranger.clone())));
        let bridge = ControlBarBridge {
            buttons_by_id: HashMap::from([(ranger.get_id(), ranger)]),
            command_sets: HashMap::from([("AmericaBarracksCommandSet".to_string(), command_set)]),
            overrides: None,
        };

        assert_eq!(
            bridge.exact_unit_build_authorization(
                "AmericaBarracksCommandSet",
                "AmericaInfantryRanger"
            ),
            Some(true)
        );
        assert_eq!(
            bridge.exact_unit_build_authorization(
                "AmericaBarracksCommandSet",
                "AmericaInfantryRangerPrototype"
            ),
            Some(false)
        );
    }

    #[test]
    fn parallel_worlds_keep_independent_control_bar_bridges() {
        use crate::system::engine_stores::{new_world_services, with_world_services};
        use std::sync::{Arc, Barrier, Mutex};
        use std::thread;

        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for seed in [1u32, 2u32] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let services = new_world_services();
                with_world_services(&services, || {
                    let mut bridge = ControlBarBridge {
                        buttons_by_id: HashMap::new(),
                        command_sets: HashMap::new(),
                        overrides: None,
                    };
                    bridge.command_sets.insert(
                        format!("Seed{seed}Set"),
                        CommandSet::new(format!("Seed{seed}Set")),
                    );
                    replace_control_bar_bridge(bridge).unwrap();
                    barrier.wait();
                    let mine = control_bar_bridge_in(&services)
                        .unwrap()
                        .find_command_set_by_name(&format!("Seed{seed}Set"))
                        .is_some();
                    let other_seed = if seed == 1 { 2 } else { 1 };
                    let other = control_bar_bridge_in(&services)
                        .unwrap()
                        .find_command_set_by_name(&format!("Seed{other_seed}Set"))
                        .is_some();
                    results.lock().unwrap().push((seed, mine, other));
                    barrier.wait();
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(s, _, _)| *s);
        assert!(got[0].1 && !got[0].2);
        assert!(got[1].1 && !got[1].2);
    }
}

#[cfg(test)]
mod owning_world_bridge_tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::Object;
    use crate::system::engine_stores::{with_world_services, EngineStores};

    fn bridge_with_set(name: &str) -> Arc<ControlBarBridge> {
        Arc::new(ControlBarBridge {
            buttons_by_id: HashMap::new(),
            command_sets: HashMap::from([(name.to_string(), CommandSet::new(name.to_string()))]),
            overrides: None,
        })
    }

    #[test]
    fn object_control_bar_reads_come_from_the_world_that_admitted_the_object() {
        // Two worlds on one thread: special-power, hunt, upgrade and
        // partition CommandSet reads (C++ TheControlBar of the GameEngine
        // owning the Object) never use the published world's ControlBar.
        let a = Arc::new(EngineStores::new_for_world());
        let b = Arc::new(EngineStores::new_for_world());
        *a.services().control_bar_bridge().write().unwrap() = Some(bridge_with_set("SetA"));
        *b.services().control_bar_bridge().write().unwrap() = Some(bridge_with_set("SetB"));
        let template = Arc::new(DefaultThingTemplate::new("BridgeOwner".to_string()));
        let in_b = Object::new_with_id(template.clone(), 81, ObjectStatusMaskType::none(), None)
            .expect("object");
        in_b.write().unwrap().bind_pathfinding_owner(&b).unwrap();
        let unadmitted =
            Object::new_with_id(template, 82, ObjectStatusMaskType::none(), None).expect("object");

        with_world_services(a.services(), || {
            let owned = control_bar_bridge_for(&in_b.read().unwrap()).expect("B's ControlBar");
            assert!(owned.find_command_set_by_name("SetB").is_some());
            assert!(owned.find_command_set_by_name("SetA").is_none());
            // An Object of no world has no ControlBar, even with A published.
            assert!(control_bar_bridge_for(&unadmitted.read().unwrap()).is_none());
        });

        // B without a ControlBar: its object sees none, not A's.
        *b.services().control_bar_bridge().write().unwrap() = None;
        with_world_services(a.services(), || {
            assert!(control_bar_bridge_for(&in_b.read().unwrap()).is_none());
        });
    }

    #[test]
    fn object_mark_ui_dirty_reaches_the_world_that_admitted_the_object() {
        // Two worlds on one thread: C++ Object::setCommandSetStringOverride,
        // capture, selection-status changes and CommandSetUpgrade mark the
        // ControlBar of the GameEngine owning the Object dirty.
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct DirtyHooks(Arc<AtomicUsize>);
        impl ControlBarUiHooks for DirtyHooks {
            fn mark_ui_dirty(&self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_player_science_purchase_points_changed(&self, _: i32, _: i32) {}
            fn on_player_rank_changed(&self, _: i32, _: i32, _: i32) {}
        }
        let a = Arc::new(EngineStores::new_for_world());
        let b = Arc::new(EngineStores::new_for_world());
        let dirty_a = Arc::new(AtomicUsize::new(0));
        let dirty_b = Arc::new(AtomicUsize::new(0));
        *a.services().control_bar_ui_hooks().lock().unwrap() =
            Some(Arc::new(DirtyHooks(Arc::clone(&dirty_a))));
        *b.services().control_bar_ui_hooks().lock().unwrap() =
            Some(Arc::new(DirtyHooks(Arc::clone(&dirty_b))));
        let template = Arc::new(DefaultThingTemplate::new("DirtyOwner".to_string()));
        let in_b = Object::new_with_id(template.clone(), 91, ObjectStatusMaskType::none(), None)
            .expect("object");
        in_b.write().unwrap().bind_pathfinding_owner(&b).unwrap();
        let unadmitted =
            Object::new_with_id(template, 92, ObjectStatusMaskType::none(), None).expect("object");

        with_world_services(a.services(), || {
            in_b.write()
                .unwrap()
                .set_command_set_string_override(&game_engine::common::ascii_string::AsciiString::from("OverrideSet"));
        });
        assert_eq!(
            (dirty_a.load(Ordering::Relaxed), dirty_b.load(Ordering::Relaxed)),
            (0, 1)
        );
        with_world_services(a.services(), || {
            mark_ui_dirty_for(&unadmitted.read().unwrap());
        });
        assert_eq!(
            (dirty_a.load(Ordering::Relaxed), dirty_b.load(Ordering::Relaxed)),
            (0, 1),
            "an Object of no world marks no ControlBar dirty"
        );
    }

}
