//! Per-world script trigger history and the geometry contract it uses.

pub mod attack_priorities;
pub mod chunk_codec;
mod control_commands;
pub mod driver;
pub mod events;
pub mod frame;
pub mod interpreter;
pub mod map_reveals;
pub mod media_conditions;
pub mod radar;
pub mod named_objects;
pub mod object_types;
pub mod player_event_conditions;
pub mod program;
pub mod program_execution;
pub mod program_install;
pub mod sequential_scripts;
pub mod side_scripts;
pub mod team_scripts;
mod variable_commands;
pub mod variables;
pub mod world;

pub use generals_game_domain::polygons::TriggerArea;
use std::collections::HashMap;

/// Result from one immediately dispatched script action.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptActionResult {
    Success,
    /// Frames remaining.
    Pending(f32),
    Failed(String),
}

/// Failure to decode or execute an authored script operation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScriptError {
    #[error("Parameter not found: {0}")]
    ParameterNotFound(String),
    #[error("Invalid parameter type: {0}")]
    InvalidParameterType(String),
    #[error("Team not found: {0}")]
    TeamNotFound(String),
    #[error("Player not found: {0}")]
    PlayerNotFound(String),
    #[error("Object not found: {0}")]
    ObjectNotFound(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Evaluation failed: {0}")]
    EvaluationFailed(String),
}

impl ScriptError {
    pub(crate) fn missing_parameter(index: usize) -> Self {
        Self::ParameterNotFound(format!("Parameter {index} not found"))
    }
}

pub const MY_INNER_PERIMETER: &str = "[Skirmish]MyInnerPerimeter";
pub const MY_OUTER_PERIMETER: &str = "[Skirmish]MyOuterPerimeter";
pub const ENEMY_INNER_PERIMETER: &str = "[Skirmish]EnemyInnerPerimeter";
pub const ENEMY_OUTER_PERIMETER: &str = "[Skirmish]EnemyOuterPerimeter";
pub const INNER_PERIMETER: &str = "InnerPerimeter";
pub const OUTER_PERIMETER: &str = "OuterPerimeter";

/// C++ ScriptEngine::getQualifiedTriggerAreaByName with no current player.
/// An enemy perimeter with no resolved enemy uses the original -1 fallback.
pub fn qualify_trigger_area_without_player(area_name: &str) -> Option<&str> {
    match area_name {
        MY_INNER_PERIMETER | MY_OUTER_PERIMETER => None,
        ENEMY_INNER_PERIMETER => Some("InnerPerimeter-1"),
        ENEMY_OUTER_PERIMETER => Some("OuterPerimeter-1"),
        name => Some(name),
    }
}

const MAX_TRIGGER_INFOS: usize = 5;

#[derive(Clone)]
struct HostTriggerSlot {
    trigger_id: i32,
    is_inside: bool,
    entered: bool,
    exited: bool,
}

struct HostObjectTriggerState {
    i_x: i32,
    i_y: i32,
    entered_or_exited_frame: u32,
    slots: Vec<HostTriggerSlot>,
}

pub struct TriggerWorld<A> {
    campaign_is_challenge: bool,
    objects: HashMap<u32, HostObjectTriggerState>,
    team_entered_or_exited: HashMap<String, u32>,
    current_frame: u32,
    triggers: Vec<A>,
    geometry_installed: bool,
}

impl<A> Default for TriggerWorld<A> {
    fn default() -> Self {
        Self {
            campaign_is_challenge: false,
            objects: HashMap::new(),
            team_entered_or_exited: HashMap::new(),
            current_frame: 0,
            triggers: Vec::new(),
            geometry_installed: false,
        }
    }
}

impl<A> std::fmt::Debug for TriggerWorld<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostTriggerWorld")
            .field("object_count", &self.objects.len())
            .field("team_event_count", &self.team_entered_or_exited.len())
            .field("current_frame", &self.current_frame)
            .field("trigger_count", &self.triggers.len())
            .field("geometry_installed", &self.geometry_installed)
            .finish()
    }
}

fn host_flag_window(flag_frame: u32, now: u32) -> bool {
    flag_frame == now || (now > 0 && flag_frame == now - 1)
}

impl<A: TriggerArea> TriggerWorld<A> {
    /// Immutable session classification admitted by the driving world before script init.
    pub fn admit_campaign_context(&mut self, is_challenge: bool) {
        self.campaign_is_challenge = is_challenge;
    }
    pub fn is_generals_challenge_campaign(&self) -> bool {
        self.campaign_is_challenge
    }

    /// Install map-authored trigger geometry for this world. The caller owns
    /// map-load ordering; no terrain singleton lookup is performed here.
    pub fn set_trigger_areas(&mut self, triggers: &[A]) {
        self.triggers.clear();
        self.triggers.extend_from_slice(triggers);
        self.geometry_installed = true;
    }

    pub fn has_authored_trigger_geometry(&self) -> bool {
        self.geometry_installed
    }

    /// Resolve a raw or already-qualified name within this world's geometry.
    /// Script qualification belongs to the driving caller's context.
    pub fn trigger_area_by_canonical_name(&self, area_name: &str) -> Option<A> {
        if area_name.is_empty() {
            return None;
        }
        self.triggers
            .iter()
            .find(|trigger| trigger.trigger_name() == area_name)
            .cloned()
    }

    pub fn trigger_area_by_name(&self, area_name: &str) -> Option<A> {
        if area_name.is_empty() {
            return None;
        }
        let resolved = qualify_trigger_area_without_player(area_name).unwrap_or(area_name);
        self.triggers
            .iter()
            .find(|trigger| {
                trigger.trigger_name() == resolved || trigger.trigger_name() == area_name
            })
            .cloned()
    }

    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// C++ Object::m_iPos, initially zero until a qualifying movement.
    pub fn object_integer_position(&self, object_id: u32) -> (i32, i32) {
        self.objects
            .get(&object_id)
            .map_or((0, 0), |state| (state.i_x, state.i_y))
    }

    /// Update trigger membership for an object in the receiving world.
    pub fn update_object_flags(
        &mut self,
        object_id: u32,
        x: f32,
        z: f32,
        frame: u32,
        skip: bool,
        team_name: Option<&str>,
    ) {
        let _ = self.update_object_flags_and_report(object_id, x, z, frame, skip, team_name);
    }

    /// Update the receiving object trigger slots and report whether this call
    /// observed an enter/exit, so Main can set its owning Team flag immediately.
    pub fn update_object_flags_and_report(
        &mut self,
        object_id: u32,
        x: f32,
        z: f32,
        frame: u32,
        skip: bool,
        team_name: Option<&str>,
    ) -> bool {
        if skip {
            return false;
        }
        let new_x = x as i32;
        let new_y = z as i32;
        let triggers = &self.triggers;
        let state = self
            .objects
            .entry(object_id)
            .or_insert_with(|| HostObjectTriggerState {
                i_x: 0,
                i_y: 0,
                entered_or_exited_frame: 0,
                slots: Vec::new(),
            });
        let pos_changed = state.i_x != new_x || state.i_y != new_y;
        // C++ Object.cpp:2575-2578: unchanged integer XY returns even with
        // zero active areas. Required so load can restore m_iPos and skip
        // a fresh ENTERED_AREA edge.
        if !pos_changed {
            return false;
        }
        if state.entered_or_exited_frame != 0 && state.entered_or_exited_frame != frame {
            state.slots.retain(|slot| slot.is_inside);
            for slot in &mut state.slots {
                slot.entered = false;
                slot.exited = false;
            }
        }
        let mut trigger_event = false;
        for slot in &mut state.slots {
            let Some(trigger) = triggers.iter().find(|t| t.trigger_id() == slot.trigger_id) else {
                continue;
            };
            if !trigger.contains_trigger_point(state.i_x, state.i_y) {
                slot.is_inside = false;
                slot.exited = true;
                state.entered_or_exited_frame = frame;
                trigger_event = true;
            }
        }
        state.i_x = new_x;
        state.i_y = new_y;
        for trigger in triggers {
            if state
                .slots
                .iter()
                .any(|slot| slot.trigger_id == trigger.trigger_id())
            {
                continue;
            }
            if !trigger.contains_trigger_point(state.i_x, state.i_y) {
                continue;
            }
            if state.slots.len() >= MAX_TRIGGER_INFOS {
                break;
            }
            state.slots.push(HostTriggerSlot {
                trigger_id: trigger.trigger_id(),
                is_inside: true,
                entered: true,
                exited: false,
            });
            state.entered_or_exited_frame = frame;
            trigger_event = true;
        }
        if trigger_event {
            if let Some(name) = team_name.filter(|name| !name.is_empty()) {
                self.team_entered_or_exited.insert(name.to_string(), frame);
            }
        }
        trigger_event
    }

    pub fn clear(&mut self) {
        self.objects.clear();
        self.team_entered_or_exited.clear();
        self.current_frame = 0;
    }

    pub fn capture(&self) -> Vec<HostObjectTriggerPersist> {
        let mut entries: Vec<HostObjectTriggerPersist> = self
            .objects
            .iter()
            .map(|(object_id, state)| HostObjectTriggerPersist {
                object_id: *object_id,
                i_x: state.i_x,
                i_y: state.i_y,
                entered_or_exited_frame: state.entered_or_exited_frame,
                slots: state
                    .slots
                    .iter()
                    .map(|slot| {
                        let trigger_name = self
                            .triggers
                            .iter()
                            .find(|trigger| trigger.trigger_id() == slot.trigger_id)
                            .map(|trigger| trigger.trigger_name().to_owned())
                            .unwrap_or_default();
                        HostTriggerSlotPersist {
                            trigger_id: slot.trigger_id,
                            trigger_name,
                            is_inside: slot.is_inside,
                            entered: slot.entered,
                            exited: slot.exited,
                        }
                    })
                    .collect(),
            })
            .collect();
        entries.sort_by_key(|entry| entry.object_id);
        entries
    }

    pub fn restore(&mut self, entries: &[HostObjectTriggerPersist]) {
        self.objects.clear();
        self.team_entered_or_exited.clear();
        for entry in entries {
            let slots = entry
                .slots
                .iter()
                .map(|slot| {
                    let trigger_id = if slot.trigger_name.is_empty() {
                        slot.trigger_id
                    } else {
                        self.triggers
                            .iter()
                            .find(|trigger| trigger.trigger_name() == slot.trigger_name)
                            .map(|trigger| trigger.trigger_id())
                            .unwrap_or(slot.trigger_id)
                    };
                    HostTriggerSlot {
                        trigger_id,
                        is_inside: slot.is_inside,
                        entered: slot.entered,
                        exited: slot.exited,
                    }
                })
                .collect();
            self.objects.insert(
                entry.object_id,
                HostObjectTriggerState {
                    i_x: entry.i_x,
                    i_y: entry.i_y,
                    entered_or_exited_frame: entry.entered_or_exited_frame,
                    slots,
                },
            );
        }
    }

    pub fn did_enter_or_exit(&self, object_id: u32, now: u32) -> bool {
        self.objects
            .get(&object_id)
            .is_some_and(|state| host_flag_window(state.entered_or_exited_frame, now))
    }

    pub fn did_enter(&self, object_id: u32, trigger: &A, now: u32) -> bool {
        self.objects.get(&object_id).is_some_and(|state| {
            host_flag_window(state.entered_or_exited_frame, now)
                && state
                    .slots
                    .iter()
                    .any(|slot| slot.entered && slot.trigger_id == trigger.trigger_id())
        })
    }

    pub fn did_exit(&self, object_id: u32, trigger: &A, now: u32) -> bool {
        self.objects.get(&object_id).is_some_and(|state| {
            host_flag_window(state.entered_or_exited_frame, now)
                && state
                    .slots
                    .iter()
                    .any(|slot| slot.exited && slot.trigger_id == trigger.trigger_id())
        })
    }

    /// C++ Object::isInside (`Object.cpp:2520-2529`): query the object's
    /// tracked trigger slot, preserving the update phase and trigger identity.
    pub fn is_inside(&self, object_id: u32, trigger: &A) -> bool {
        self.objects.get(&object_id).is_some_and(|state| {
            state
                .slots
                .iter()
                .any(|slot| slot.is_inside && slot.trigger_id == trigger.trigger_id())
        })
    }

    /// The caller supplies team membership from its own world when needed.
    pub fn team_did_enter_or_exit(
        &self,
        team_name: &str,
        now: u32,
        members: impl FnOnce() -> Vec<u32>,
    ) -> bool {
        self.team_entered_or_exited
            .get(team_name)
            .copied()
            .is_some_and(|frame| host_flag_window(frame, now))
            || members()
                .into_iter()
                .any(|id| self.did_enter_or_exit(id, now))
    }
}

/// C++ `Object::xfer` (`Object.cpp:4218-4246`) per-area slot.
#[derive(Clone, Debug, Default)]
pub struct HostTriggerSlotPersist {
    pub trigger_id: i32,
    pub trigger_name: String,
    pub is_inside: bool,
    pub entered: bool,
    pub exited: bool,
}

/// C++ `Object::xfer` trigger housekeeping: `m_iPos`, `m_enteredOrExitedFrame`,
/// `m_numTriggerAreasActive` + per-area entered/exited/isInside.
#[derive(Clone, Debug, Default)]
pub struct HostObjectTriggerPersist {
    pub object_id: u32,
    pub i_x: i32,
    pub i_y: i32,
    pub entered_or_exited_frame: u32,
    pub slots: Vec<HostTriggerSlotPersist>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Area {
        id: i32,
        name: &'static str,
    }

    impl TriggerArea for Area {
        fn trigger_id(&self) -> i32 {
            self.id
        }

        fn trigger_name(&self) -> &str {
            self.name
        }

        fn contains_trigger_point(&self, x: i32, y: i32) -> bool {
            (4..10).contains(&x) && (2..6).contains(&y)
        }
    }

    #[test]
    fn trigger_history_restores_names_and_observes_the_original_exit_phase() {
        let old_area = Area {
            id: 4,
            name: "Depot",
        };
        let new_area = Area {
            id: 9,
            name: "Depot",
        };
        let mut world = TriggerWorld::default();
        world.set_trigger_areas(&[old_area.clone()]);
        assert!(world.update_object_flags_and_report(11, 5.8, 3.4, 22, false, Some(" convoy")));
        assert!(world.did_enter(11, &old_area, 23));
        assert!(!world.did_enter(11, &old_area, 24));

        let mut restored = TriggerWorld::default();
        restored.set_trigger_areas(&[new_area.clone()]);
        restored.set_current_frame(23);
        restored.restore(&world.capture());
        assert_eq!(restored.current_frame(), 23);
        assert_eq!(restored.object_integer_position(11), (5, 3));
        assert!(restored.is_inside(11, &new_area));
        assert!(!restored.is_inside(11, &old_area));

        assert!(!restored.update_object_flags_and_report(11, 12.0, 3.0, 23, false, None));
        assert!(restored.is_inside(11, &new_area));
        assert!(restored.update_object_flags_and_report(11, 13.0, 3.0, 24, false, None));
        assert!(restored.did_exit(11, &new_area, 25));
        assert!(world.is_inside(11, &old_area));
    }

    #[test]
    fn trigger_admission_preserves_polygon_order_and_the_five_slot_limit() {
        let mut world = TriggerWorld::default();
        let literal = Area {
            id: 1,
            name: ENEMY_INNER_PERIMETER,
        };
        let qualified = Area {
            id: 2,
            name: "InnerPerimeter-1",
        };
        let mut areas = vec![literal.clone(), qualified.clone()];
        areas.extend((3..=7).map(|id| Area { id, name: "Area" }));
        world.set_trigger_areas(&areas);
        assert_eq!(
            world
                .trigger_area_by_name(ENEMY_INNER_PERIMETER)
                .unwrap()
                .id,
            literal.id
        );
        assert_eq!(
            world
                .trigger_area_by_canonical_name("InnerPerimeter-1")
                .unwrap()
                .id,
            qualified.id
        );
        world.update_object_flags(20, 6.0, 3.0, 5, false, Some("Patrol"));
        let snapshots = world.capture();
        let ids: Vec<_> = snapshots[0]
            .slots
            .iter()
            .map(|slot| slot.trigger_id)
            .collect();
        assert_eq!(ids, [1, 2, 3, 4, 5]);
        assert!(world.team_did_enter_or_exit("Patrol", 6, || panic!("team edge is already known")));
        assert!(world.team_did_enter_or_exit("Other", 6, || vec![20]));
    }
}
