//! Post-logic identity residuals: command-set, model, formation, FOW, kind-of, mesh.

use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;
use super::*;

// Wave 696: post-logic command-set / disguise / vision-camo batch handoff.
thread_local! {
    static EARLY_COMMAND_SET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_command_set_log::HostCommandSetEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_command_set_batch() -> Option<(
    Vec<crate::game_logic::host_command_set_log::HostCommandSetEvent>,
    bool,
)> {
    EARLY_COMMAND_SET_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 696: post-logic drain `host_command_set_log` into GameWorld SetCommandSet.
pub fn eager_apply_host_command_set_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic command-set materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_command_set_log::drain();
    if events.is_empty() {
        EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_command_set_events(&events);
    EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 697: post-logic weapon-stats / selection-radius / model-condition batch handoff.
thread_local! {
    static EARLY_SELECTION_RADIUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 697: post-logic weapon-stats / selection-radius / model-condition batch handoff.
thread_local! {
    static EARLY_MODEL_CONDITION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_model_condition_log::HostModelConditionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_selection_radius_batch() -> Option<(
    Vec<crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent>,
    bool,
)> {
    EARLY_SELECTION_RADIUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_model_condition_batch() -> Option<(
    Vec<crate::game_logic::host_model_condition_log::HostModelConditionEvent>,
    bool,
)> {
    EARLY_MODEL_CONDITION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 697: post-logic drain `host_selection_radius_log` into GameWorld SetSelectionRadius.
pub fn eager_apply_host_selection_radius_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic selection-radius materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_selection_radius_log::drain();
    if events.is_empty() {
        EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_selection_radius_events(&events);
    EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 697: post-logic drain `host_model_condition_log` into GameWorld SetModelCondition.
pub fn eager_apply_host_model_condition_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic model-condition materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_model_condition_log::drain();
    if events.is_empty() {
        EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_model_condition_events(&events);
    EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 698: post-logic demo-mine-cheer / formation / crush-vision batch handoff.
thread_local! {
    static EARLY_DEMO_MINE_CHEER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 698: post-logic demo-mine-cheer / formation / crush-vision batch handoff.
thread_local! {
    static EARLY_FORMATION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_formation_log::HostFormationEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 698: post-logic demo-mine-cheer / formation / crush-vision batch handoff.
thread_local! {
    static EARLY_CRUSH_VISION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_crush_vision_log::HostCrushVisionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_demo_mine_cheer_batch() -> Option<(
    Vec<crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent>,
    bool,
)> {
    EARLY_DEMO_MINE_CHEER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_formation_batch() -> Option<(
    Vec<crate::game_logic::host_formation_log::HostFormationEvent>,
    bool,
)> {
    EARLY_FORMATION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_crush_vision_batch() -> Option<(
    Vec<crate::game_logic::host_crush_vision_log::HostCrushVisionEvent>,
    bool,
)> {
    EARLY_CRUSH_VISION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 698: post-logic drain `host_demo_mine_cheer_log` into GameWorld SetDemoMineCheer.
pub fn eager_apply_host_demo_mine_cheer_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic demo-mine-cheer materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_demo_mine_cheer_log::drain();
    if events.is_empty() {
        EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_demo_mine_cheer_events(&events);
    EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 698: post-logic drain `host_formation_log` into GameWorld SetFormation.
pub fn eager_apply_host_formation_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic formation materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_formation_log::drain();
    if events.is_empty() {
        EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_formation_events(&events);
    EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 698: post-logic drain `host_crush_vision_log` into GameWorld SetCrushVision.
pub fn eager_apply_host_crush_vision_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic crush-vision materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_crush_vision_log::drain();
    if events.is_empty() {
        EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_crush_vision_events(&events);
    EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 699: post-logic building-type / identity / ground-height batch handoff.
thread_local! {
    static EARLY_BUILDING_TYPE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_building_type_log::HostBuildingTypeEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 699: post-logic building-type / identity / ground-height batch handoff.
thread_local! {
    static EARLY_IDENTITY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_identity_log::HostIdentityEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 699: post-logic building-type / identity / ground-height batch handoff.
thread_local! {
    static EARLY_GROUND_HEIGHT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ground_height_log::HostGroundHeightEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_building_type_batch() -> Option<(
    Vec<crate::game_logic::host_building_type_log::HostBuildingTypeEvent>,
    bool,
)> {
    EARLY_BUILDING_TYPE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_identity_batch() -> Option<(
    Vec<crate::game_logic::host_identity_log::HostIdentityEvent>,
    bool,
)> {
    EARLY_IDENTITY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_ground_height_batch() -> Option<(
    Vec<crate::game_logic::host_ground_height_log::HostGroundHeightEvent>,
    bool,
)> {
    EARLY_GROUND_HEIGHT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 699: post-logic drain `host_building_type_log` into GameWorld SetBuildingType.
pub fn eager_apply_host_building_type_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic building-type materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_building_type_log::drain();
    if events.is_empty() {
        EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_building_type_events(&events);
    EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 699: post-logic drain `host_identity_log` into GameWorld SetIdentity.
pub fn eager_apply_host_identity_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic identity materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_identity_log::drain();
    if events.is_empty() {
        EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_identity_events(&events);
    EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 699: post-logic drain `host_ground_height_log` into GameWorld SetGroundHeight.
pub fn eager_apply_host_ground_height_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic ground-height materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ground_height_log::drain();
    if events.is_empty() {
        EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ground_height_events(&events);
    EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 700: post-logic model-mesh / fow / kind-of batch handoff.
thread_local! {
    static EARLY_MODEL_MESH_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_model_mesh_log::HostModelMeshEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 700: post-logic model-mesh / fow / kind-of batch handoff.
thread_local! {
    static EARLY_FOW_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_fow_log::HostFowEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 700: post-logic model-mesh / fow / kind-of batch handoff.
thread_local! {
    static EARLY_KIND_OF_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_kind_of_log::HostKindOfEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_model_mesh_batch() -> Option<(
    Vec<crate::game_logic::host_model_mesh_log::HostModelMeshEvent>,
    bool,
)> {
    EARLY_MODEL_MESH_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fow_batch() -> Option<(Vec<crate::game_logic::host_fow_log::HostFowEvent>, bool)> {
    EARLY_FOW_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_kind_of_batch() -> Option<(
    Vec<crate::game_logic::host_kind_of_log::HostKindOfEvent>,
    bool,
)> {
    EARLY_KIND_OF_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 700: post-logic drain `host_model_mesh_log` into GameWorld SetModelMesh.
pub fn eager_apply_host_model_mesh_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic model-mesh materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_model_mesh_log::drain();
    if events.is_empty() {
        EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_model_mesh_events(&events);
    EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 700: post-logic drain `host_fow_log` into GameWorld SetFow.
pub fn eager_apply_host_fow_after_logic(shadow: &mut GameWorldShadow, _logic: &GameLogic) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic FOW materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_fow_log::drain();
    if events.is_empty() {
        EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fow_events(&events);
    EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 700: post-logic drain `host_kind_of_log` into GameWorld SetKindOfBits.
pub fn eager_apply_host_kind_of_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic kind-of materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_kind_of_log::drain();
    if events.is_empty() {
        EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_kind_of_events(&events);
    EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_identity_batches() {
    EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = None);
}
