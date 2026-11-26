//! Ordered side-script ownership and immediate subroutine dispatch.
//! ScriptEngine.cpp:6269–6308, 6797–6865 and 7653–7667.

mod continuation;

pub use continuation::SideScriptContinuation;

use super::program::{Script, ScriptGroup, ScriptList};
use crate::{GameLogicError, GameLogicResult};
use std::cell::{Cell, RefCell};

/// A location in the C++ `ScriptList` linked-list layout.  Keeping the
/// location rather than a pointer lets a script be removed temporarily while
/// its action chain is dispatched, then reinserted at exactly the same spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScriptLocation {
    side_index: usize,
    container: ScriptContainer,
    script_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptContainer {
    Root,
    Group(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GroupLocation {
    side_index: usize,
    group_index: usize,
}

/// Metadata retained while a Script is detached for immediate execution.
/// The script itself remains owned by the executing stack frame, so this
/// record never manufactures a second mutable reference to it.
#[derive(Debug)]
struct ExecutingScript {
    location: ScriptLocation,
    name: String,
    is_subroutine: bool,
}

struct ListState {
    lists: Vec<Option<Box<ScriptList>>>,
    executing_scripts: Vec<ExecutingScript>,
    executing_groups: Vec<GroupLocation>,
}

/// Result of an exact C++ `findGroup` then `findScript` lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubroutineLookup {
    Group {
        location: GroupLocation,
        is_subroutine: bool,
        is_active: bool,
    },
    Script {
        location: ScriptLocation,
        is_subroutine: bool,
    },
    /// The matching script is currently executing in this immediate call
    /// stack.  It is deliberately distinguished from `Missing`: C++ would
    /// recurse through the same object, which cannot be represented with two
    /// safe mutable borrows of this linked-list node.
    ReentrantScript {
        location: ScriptLocation,
        is_subroutine: bool,
    },
    Missing,
}

/// Temporarily owns one executing Script.  Dropping it reinstalls the node at
/// the original linked-list position before its enclosing list store restores
/// to `ScriptEngineInner`.
struct DetachedScript<'a> {
    active_lists: &'a std::cell::RefCell<ListState>,
    location: ScriptLocation,
    script: Option<Box<Script>>,
}

impl<'a> DetachedScript<'a> {
    fn take(
        active_lists: &'a std::cell::RefCell<ListState>,
        location: ScriptLocation,
    ) -> Option<Self> {
        let mut active_lists_mut = active_lists.borrow_mut();
        let script = SideScripts::take_script_at_location(&mut active_lists_mut, location)?;
        let executing = ExecutingScript {
            location,
            name: script.script_name.clone(),
            is_subroutine: script.is_subroutine,
        };
        active_lists_mut.executing_scripts.push(executing);
        drop(active_lists_mut);
        Some(Self {
            active_lists,
            location,
            script: Some(script),
        })
    }

    fn script_mut(&mut self) -> &mut Script {
        self.script
            .as_deref_mut()
            .expect("DetachedScript always owns its Script until Drop")
    }
}

impl Drop for DetachedScript<'_> {
    fn drop(&mut self) {
        let Some(script) = self.script.take() else {
            return;
        };
        let mut active_lists = self.active_lists.borrow_mut();
        if !SideScripts::restore_script_at_location(&mut active_lists, self.location, script) {
            // Replacing/clearing a side list while one of its scripts is on the
            // native-style execution stack is invalid in C++ as well.  Keep it
            // loud rather than pretending the script was never present.
            log::error!(
                "lost detached Script at side {}, location {:?} while restoring immediate execution",
                self.location.side_index,
                self.location
            );
        }
        active_lists
            .executing_scripts
            .retain(|executing| executing.location != self.location);
    }
}

/// Marks a subroutine group as executing without borrowing the group over an
/// action dispatch.  A nested call to that exact group is reported explicitly
/// instead of being accidentally invisible.
struct ExecutingGroupGuard<'a> {
    active_lists: &'a std::cell::RefCell<ListState>,
    location: GroupLocation,
}

impl Drop for ExecutingGroupGuard<'_> {
    fn drop(&mut self) {
        self.active_lists
            .borrow_mut()
            .executing_groups
            .retain(|location| *location != self.location);
    }
}

/// Hard cap on nested CALL_SUBROUTINE depth (C++ is stack-bound; we fail closed).
const MAX_SUBROUTINE_CALL_DEPTH: u32 = 32;

struct SubroutineDepthGuard<'a> {
    depth: &'a Cell<u32>,
}

impl<'a> SubroutineDepthGuard<'a> {
    fn enter(depth: &'a Cell<u32>) -> Option<Self> {
        let current = depth.get();
        if current >= MAX_SUBROUTINE_CALL_DEPTH {
            log::warn!(
                "CALL_SUBROUTINE depth limit ({}) exceeded; aborting nested call",
                MAX_SUBROUTINE_CALL_DEPTH
            );
            None
        } else {
            depth.set(current + 1);
            Some(Self { depth })
        }
    }
}

impl Drop for SubroutineDepthGuard<'_> {
    fn drop(&mut self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }
}

struct ExecutionGuard<'a>(&'a Cell<bool>);

impl Drop for ExecutionGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Interpreter-owned side lists and the synchronous re-entry state.
pub struct SideScripts {
    state: RefCell<ListState>,
    executing: Cell<bool>,
    subroutine_depth: Cell<u32>,
}

impl SideScripts {
    pub fn new(side_count: usize) -> Self {
        Self {
            state: RefCell::new(ListState {
                lists: vec![None; side_count],
                executing_scripts: Vec::new(),
                executing_groups: Vec::new(),
            }),
            executing: Cell::new(false),
            subroutine_depth: Cell::new(0),
        }
    }

    pub fn install(
        &self,
        side: usize,
        mut list: Option<Box<ScriptList>>,
        initialize: impl FnOnce(&mut ScriptList),
    ) -> GameLogicResult<()> {
        if side >= self.state.borrow().lists.len() {
            return Err(GameLogicError::Configuration(format!(
                "Player index {} out of range for ScriptEngine",
                side
            )));
        }
        if let Some(list) = list.as_deref_mut() {
            initialize(list);
        }
        self.state.borrow_mut().lists[side] = list;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.state.get_mut().lists.fill(None);
    }

    pub fn set_active(&self, name: &str, active: bool) -> bool {
        Self::set_script_or_group_active_in_lists(&mut self.state.borrow_mut().lists, name, active)
    }

    pub fn find_script(
        &self,
        name: &str,
        require_subroutine: bool,
        copy_script: fn(&Script) -> Script,
    ) -> Option<Script> {
        self.state
            .borrow()
            .lists
            .iter()
            .filter_map(Option::as_deref)
            .find_map(|list| {
                Self::find_script_in_list_with(list, name, require_subroutine, copy_script)
            })
    }

    /// Keep saves outside the active script walk, including its driver callbacks.
    /// Nested execution on this instance shares the same list and node positions.
    pub fn with_execution<R>(&self, execute: impl FnOnce() -> R) -> R {
        if self.executing.replace(true) {
            return execute();
        }
        let _guard = ExecutionGuard(&self.executing);
        execute()
    }

    /// Visit the side's root scripts, then its active non-subroutine groups.
    /// Re-read the next group after dispatch because an action may toggle it.
    pub fn execute_side(
        &self,
        side_index: usize,
        mut execute: impl FnMut(&mut Script) -> GameLogicResult<()>,
    ) -> GameLogicResult<()> {
        self.with_execution(|| {
            self.execute_non_subroutine_scripts_in_container(
                side_index,
                ScriptContainer::Root,
                &mut execute,
            )?;
            let mut group_index = 0;
            while let Some((active, subroutine)) = self.group_state_at(GroupLocation {
                side_index,
                group_index,
            }) {
                if active && !subroutine {
                    self.execute_non_subroutine_scripts_in_container(
                        side_index,
                        ScriptContainer::Group(group_index),
                        &mut execute,
                    )?;
                }
                group_index += 1;
            }
            Ok(())
        })
    }

    pub fn call_subroutine(
        &self,
        name: &str,
        mut execute: impl FnMut(&mut Script) -> GameLogicResult<()>,
    ) -> GameLogicResult<bool> {
        self.with_execution(|| {
            let Some(_depth) = SubroutineDepthGuard::enter(&self.subroutine_depth) else {
                return Ok(false);
            };
            self.call_in_execution(name, &mut execute)
        })
    }

    /// C++ `findScript` returns the first declaration in side/root/group
    /// traversal order, so a toggle must stop after the first matching script.
    fn set_script_active_in_chain(script: &mut Script, name: &str, active: bool) -> bool {
        let mut current: Option<&mut Script> = Some(script);
        while let Some(script_ref) = current {
            if script_ref.script_name == name {
                script_ref.is_active = active;
                return true;
            }
            current = script_ref.next_script.as_deref_mut();
        }
        false
    }

    /// Match C++ `findScript`: root scripts first, followed by groups in
    /// declaration order, with the first match winning.
    fn set_script_active_in_list(list: &mut ScriptList, name: &str, active: bool) -> bool {
        if let Some(script_head) = list.first_script.as_deref_mut() {
            if Self::set_script_active_in_chain(script_head, name, active) {
                return true;
            }
        }

        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                if Self::set_script_active_in_chain(script_head, name, active) {
                    return true;
                }
            }
            group_opt = group.next_group.as_deref_mut();
        }

        false
    }

    /// Match C++ `findGroup`: side order, then group declaration order, with
    /// the first matching group winning.
    fn set_group_active_in_list(list: &mut ScriptList, name: &str, active: bool) -> bool {
        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if group.group_name == name {
                group.is_group_active = active;
                return true;
            }
            group_opt = group.next_group.as_deref_mut();
        }
        false
    }

    fn set_script_active_in_lists(
        lists: &mut [Option<Box<ScriptList>>],
        name: &str,
        active: bool,
    ) -> bool {
        for slot in lists {
            let Some(list) = slot.as_deref_mut() else {
                continue;
            };
            if Self::set_script_active_in_list(list, name, active) {
                return true;
            }
        }
        false
    }

    fn set_group_active_in_lists(
        lists: &mut [Option<Box<ScriptList>>],
        name: &str,
        active: bool,
    ) -> bool {
        for slot in lists {
            let Some(list) = slot.as_deref_mut() else {
                continue;
            };
            if Self::set_group_active_in_list(list, name, active) {
                return true;
            }
        }
        false
    }

    /// C++ `enableScript` finds a group then a script; `disableScript` does
    /// the inverse.  They are independent lookups, so a same-named group and
    /// script both change state in the original ordering.
    fn set_script_or_group_active_in_lists(
        lists: &mut [Option<Box<ScriptList>>],
        name: &str,
        active: bool,
    ) -> bool {
        if active {
            let group_updated = Self::set_group_active_in_lists(lists, name, true);
            let script_updated = Self::set_script_active_in_lists(lists, name, true);
            group_updated || script_updated
        } else {
            let script_updated = Self::set_script_active_in_lists(lists, name, false);
            let group_updated = Self::set_group_active_in_lists(lists, name, false);
            group_updated || script_updated
        }
    }

    fn find_script_in_chain_with(
        script: &Script,
        name: &str,
        require_subroutine: bool,
        copy_script: fn(&Script) -> Script,
    ) -> Option<Script> {
        let mut current: Option<&Script> = Some(script);
        while let Some(script_ref) = current {
            if script_ref.script_name == name && (!require_subroutine || script_ref.is_subroutine) {
                return Some(copy_script(script_ref));
            }
            current = script_ref.next_script.as_deref();
        }
        None
    }

    fn find_script_in_list_with(
        list: &ScriptList,
        name: &str,
        require_subroutine: bool,
        copy_script: fn(&Script) -> Script,
    ) -> Option<Script> {
        if let Some(script_head) = list.first_script.as_deref() {
            if let Some(found) =
                Self::find_script_in_chain_with(script_head, name, require_subroutine, copy_script)
            {
                return Some(found);
            }
        }

        let mut group_opt = list.first_group.as_deref();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref() {
                if let Some(found) = Self::find_script_in_chain_with(
                    script_head,
                    name,
                    require_subroutine,
                    copy_script,
                ) {
                    return Some(found);
                }
            }
            group_opt = group.next_group.as_deref();
        }

        None
    }

    fn group_at_in_list(list: &ScriptList, group_index: usize) -> Option<&ScriptGroup> {
        let mut group = list.first_group.as_deref();
        for _ in 0..group_index {
            group = group?.next_group.as_deref();
        }
        group
    }

    fn group_at_in_list_mut(list: &mut ScriptList, group_index: usize) -> Option<&mut ScriptGroup> {
        let mut group = list.first_group.as_deref_mut();
        for _ in 0..group_index {
            group = group?.next_group.as_deref_mut();
        }
        group
    }

    fn script_head_in_container(list: &ScriptList, container: ScriptContainer) -> Option<&Script> {
        match container {
            ScriptContainer::Root => list.first_script.as_deref(),
            ScriptContainer::Group(group_index) => Self::group_at_in_list(list, group_index)?
                .first_script
                .as_deref(),
        }
    }

    fn script_head_in_container_mut(
        list: &mut ScriptList,
        container: ScriptContainer,
    ) -> Option<&mut Option<Box<Script>>> {
        match container {
            ScriptContainer::Root => Some(&mut list.first_script),
            ScriptContainer::Group(group_index) => {
                Some(&mut Self::group_at_in_list_mut(list, group_index)?.first_script)
            }
        }
    }

    fn script_at_in_chain(mut script: Option<&Script>, script_index: usize) -> Option<&Script> {
        for _ in 0..script_index {
            script = script?.next_script.as_deref();
        }
        script
    }

    fn script_link_at_mut(
        head: &mut Option<Box<Script>>,
        script_index: usize,
    ) -> Option<&mut Option<Box<Script>>> {
        let mut link = head;
        for _ in 0..script_index {
            link = &mut link.as_deref_mut()?.next_script;
        }
        Some(link)
    }

    fn script_count_in_chain(mut script: Option<&Script>) -> usize {
        let mut count = 0;
        while let Some(current) = script {
            count += 1;
            script = current.next_script.as_deref();
        }
        count
    }

    fn detached_before(active_lists: &ListState, location: ScriptLocation) -> usize {
        active_lists
            .executing_scripts
            .iter()
            .filter(|executing| {
                executing.location.side_index == location.side_index
                    && executing.location.container == location.container
                    && executing.location.script_index < location.script_index
            })
            .count()
    }

    fn is_script_detached(active_lists: &ListState, location: ScriptLocation) -> bool {
        active_lists
            .executing_scripts
            .iter()
            .any(|executing| executing.location == location)
    }

    fn stored_script_at_location<'a>(
        active_lists: &'a ListState,
        location: ScriptLocation,
    ) -> Option<&'a Script> {
        if Self::is_script_detached(active_lists, location) {
            return None;
        }
        let list = active_lists.lists.get(location.side_index)?.as_deref()?;
        let script_index = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))?;
        Self::script_at_in_chain(
            Self::script_head_in_container(list, location.container),
            script_index,
        )
    }

    fn script_count_in_container(
        active_lists: &ListState,
        side_index: usize,
        container: ScriptContainer,
    ) -> usize {
        let stored = active_lists
            .lists
            .get(side_index)
            .and_then(|slot| slot.as_deref())
            .map(|list| {
                Self::script_count_in_chain(Self::script_head_in_container(list, container))
            })
            .unwrap_or(0);
        let detached = active_lists
            .executing_scripts
            .iter()
            .filter(|executing| {
                executing.location.side_index == side_index
                    && executing.location.container == container
            })
            .count();
        stored + detached
    }

    fn take_script_at_location(
        active_lists: &mut ListState,
        location: ScriptLocation,
    ) -> Option<Box<Script>> {
        if Self::is_script_detached(active_lists, location) {
            return None;
        }
        let stored_index = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))?;
        let list = active_lists
            .lists
            .get_mut(location.side_index)?
            .as_deref_mut()?;
        let link = Self::script_link_at_mut(
            Self::script_head_in_container_mut(list, location.container)?,
            stored_index,
        )?;
        let mut script = link.take()?;
        *link = script.next_script.take();
        Some(script)
    }

    fn restore_script_at_location(
        active_lists: &mut ListState,
        location: ScriptLocation,
        mut script: Box<Script>,
    ) -> bool {
        let Some(stored_index) = location
            .script_index
            .checked_sub(Self::detached_before(active_lists, location))
        else {
            return false;
        };
        let Some(list) = active_lists
            .lists
            .get_mut(location.side_index)
            .and_then(|slot| slot.as_deref_mut())
        else {
            return false;
        };
        let Some(link) = Self::script_link_at_mut(
            match Self::script_head_in_container_mut(list, location.container) {
                Some(head) => head,
                None => return false,
            },
            stored_index,
        ) else {
            return false;
        };
        script.next_script = link.take();
        *link = Some(script);
        true
    }

    fn find_script_in_chain(
        active_lists: &ListState,
        side_index: usize,
        container: ScriptContainer,
        name: &str,
    ) -> Option<SubroutineLookup> {
        let script_count = Self::script_count_in_container(active_lists, side_index, container);
        for script_index in 0..script_count {
            let location = ScriptLocation {
                side_index,
                container,
                script_index,
            };
            if let Some(executing) = active_lists
                .executing_scripts
                .iter()
                .find(|executing| executing.location == location)
            {
                if executing.name == name {
                    return Some(SubroutineLookup::ReentrantScript {
                        location,
                        is_subroutine: executing.is_subroutine,
                    });
                }
                continue;
            }
            let Some(script) = Self::stored_script_at_location(active_lists, location) else {
                continue;
            };
            if script.script_name == name {
                return Some(SubroutineLookup::Script {
                    location,
                    is_subroutine: script.is_subroutine,
                });
            }
        }
        None
    }

    fn find_subroutine_lookup(&self, name: &str) -> SubroutineLookup {
        let active_lists = &*self.state.borrow();

        // C++ `findGroup` scans all sides before `callSubroutine` ever
        // calls `findScript`; retain that global precedence exactly.
        for side_index in 0..active_lists.lists.len() {
            let Some(list) = active_lists.lists[side_index].as_deref() else {
                continue;
            };
            let mut group = list.first_group.as_deref();
            let mut group_index = 0;
            while let Some(current) = group {
                if current.group_name == name {
                    return SubroutineLookup::Group {
                        location: GroupLocation {
                            side_index,
                            group_index,
                        },
                        is_subroutine: current.is_group_subroutine,
                        is_active: current.is_group_active,
                    };
                }
                group = current.next_group.as_deref();
                group_index += 1;
            }
        }

        // C++ `findScript` scans root scripts then group scripts for each
        // side, with sides in ascending index order.
        for side_index in 0..active_lists.lists.len() {
            let Some(list) = active_lists.lists[side_index].as_deref() else {
                continue;
            };
            if let Some(found) =
                Self::find_script_in_chain(active_lists, side_index, ScriptContainer::Root, name)
            {
                return found;
            }

            let mut group = list.first_group.as_deref();
            let mut group_index = 0;
            while let Some(current) = group {
                if let Some(found) = Self::find_script_in_chain(
                    active_lists,
                    side_index,
                    ScriptContainer::Group(group_index),
                    name,
                ) {
                    return found;
                }
                group = current.next_group.as_deref();
                group_index += 1;
            }
        }

        SubroutineLookup::Missing
    }

    fn with_detached_script<R>(
        &self,
        location: ScriptLocation,
        execute: impl FnOnce(&mut Script) -> GameLogicResult<R>,
    ) -> GameLogicResult<R> {
        let mut script = DetachedScript::take(&self.state, location).ok_or_else(|| {
            GameLogicError::Configuration(format!(
                "Script at side {}, location {:?} disappeared during immediate execution",
                location.side_index, location
            ))
        })?;
        execute(script.script_mut())
    }

    fn with_executing_subroutine_group<R>(
        &self,
        location: GroupLocation,
        execute: impl FnOnce() -> GameLogicResult<R>,
    ) -> GameLogicResult<R> {
        {
            let mut state = self.state.borrow_mut();
            if state.executing_groups.contains(&location) {
                return Err(GameLogicError::Configuration(format!(
                    "recursive CALL_SUBROUTINE group at side {}, group {} is rejected by the safe re-entry boundary",
                    location.side_index, location.group_index
                )));
            }
            state.executing_groups.push(location);
        }
        let _group = ExecutingGroupGuard {
            active_lists: &self.state,
            location,
        };
        execute()
    }

    fn script_slot_at(&self, location: ScriptLocation) -> Option<(bool, bool)> {
        let state = self.state.borrow();
        if let Some(executing) = state
            .executing_scripts
            .iter()
            .find(|executing| executing.location == location)
        {
            return Some((executing.is_subroutine, true));
        }
        Self::stored_script_at_location(&state, location)
            .map(|script| (script.is_subroutine, false))
    }

    fn group_state_at(&self, location: GroupLocation) -> Option<(bool, bool)> {
        let state = self.state.borrow();
        let list = state.lists.get(location.side_index)?.as_deref()?;
        let group = Self::group_at_in_list(list, location.group_index)?;
        Some((group.is_group_active, group.is_group_subroutine))
    }

    /// Executes an entire linked Script chain without borrowing the chain over
    /// dispatch.  Each Script is detached/reinserted before its action chain,
    /// so sibling and other-side subroutines stay visible immediately.
    fn execute_non_subroutine_scripts_in_container(
        &self,
        side_index: usize,
        container: ScriptContainer,
        execute: &mut impl FnMut(&mut Script) -> GameLogicResult<()>,
    ) -> GameLogicResult<()> {
        let mut script_index = 0;
        while let Some((is_subroutine, is_executing)) = self.script_slot_at(ScriptLocation {
            side_index,
            container,
            script_index,
        }) {
            if !is_subroutine && !is_executing {
                self.with_detached_script(
                    ScriptLocation {
                        side_index,
                        container,
                        script_index,
                    },
                    &mut *execute,
                )?;
            }
            script_index += 1;
        }
        Ok(())
    }

    fn call_in_execution(
        &self,
        name: &str,
        execute: &mut impl FnMut(&mut Script) -> GameLogicResult<()>,
    ) -> GameLogicResult<bool> {
        match self.find_subroutine_lookup(name) {
            SubroutineLookup::Group {
                location,
                is_subroutine,
                is_active,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: group '{}' exists but is not a subroutine group",
                        name
                    );
                    return Ok(true);
                }
                if is_active {
                    self.with_executing_subroutine_group(location, || {
                        self.execute_non_subroutine_scripts_in_container(
                            location.side_index,
                            ScriptContainer::Group(location.group_index),
                            execute,
                        )
                    })?;
                }
                Ok(true)
            }
            SubroutineLookup::Script {
                location,
                is_subroutine,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: script '{}' exists but is not a subroutine",
                        name
                    );
                    return Ok(true);
                }
                self.with_detached_script(location, execute)?;
                Ok(true)
            }
            SubroutineLookup::ReentrantScript {
                location,
                is_subroutine,
            } => {
                if !is_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: active script '{}' exists but is not a subroutine",
                        name
                    );
                    return Ok(true);
                }
                Err(GameLogicError::Configuration(format!(
                    "recursive CALL_SUBROUTINE '{}' at side {}, script {} is rejected by the safe re-entry boundary",
                    name, location.side_index, location.script_index
                )))
            }
            SubroutineLookup::Missing => {
                log::warn!("CALL_SUBROUTINE: script '{}' is not defined", name);
                Ok(false)
            }
        }
    }
}
