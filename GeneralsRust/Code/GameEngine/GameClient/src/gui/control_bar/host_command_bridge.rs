//! Typed commands emitted by the live Control Bar for the authoritative host.
//!
//! The standalone GameClient historically feeds its commands into GameLogic's
//! global command queue.  The Rust executable has its own authoritative world,
//! so doing that there would make a HUD click look accepted while changing a
//! different simulation.  This small bridge is deliberately opt-in: normal
//! GameClient users retain the legacy queue path, while the host drains these
//! typed requests and applies them to its own world.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use gamelogic::commands::CommandType;

use super::{CommandButton, CommandSourceType, ControlBarContext, QueueProductionType};

/// A target mode armed by a command-button click.
///
/// The outer [`HostControlBarRequest::ArmTarget`] retains the button command
/// name, legacy command type, options, selected IDs, player and source.  This
/// enum adds the target-specific identity needed by the Rust host without
/// relying on legacy GameLogic globals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostControlBarTarget {
    /// A dozer is waiting for the player to place a named building template.
    DozerConstruct {
        /// INI object/template name, e.g. `AmericaPowerPlant`.
        template_name: String,
    },
    /// A named special power is waiting for an object or map target.
    SpecialPower {
        /// INI special-power identity, retained even when no numeric ID exists.
        special_power_name: String,
        /// Legacy special-power ID when the GameLogic command-button bridge has it.
        special_power_id: Option<u32>,
    },
    /// A non-building, non-special command that needs a target.
    Generic,
}

/// Provenance carried with a host Control Bar request.
///
/// `CommandSourceType::FromUser` describes the C++ gameplay source, but it
/// does not prove that a real OS event produced the click: runtime-host input
/// injection legitimately uses the same source type.  This separate value is
/// set only by the synchronous WND-dispatch scope owned by Main.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostControlBarInputProvenance {
    /// The request was published while handling a real
    /// `winit::WindowEvent::MouseInput`.
    PhysicalWindowMouseInput,
    /// There was no physical WND-dispatch scope, or the current scope was an
    /// injected/test input path.  This intentionally fails closed.
    InjectedOrUnknown,
}

impl HostControlBarInputProvenance {
    /// Whether this request came directly from a real OS mouse-input event.
    #[inline]
    pub fn is_physical_window_mouse_input(self) -> bool {
        matches!(self, Self::PhysicalWindowMouseInput)
    }
}

/// The mouse button which activated the Control Bar's retail LeftHUD minimap.
///
/// This is deliberately independent of winit's button enum: GameClient only
/// exposes the two button messages accepted by `LeftHUDInput`, while Main
/// decides whether the current alternate-mouse setting turns that click into
/// a camera pan or an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMinimapMouseButton {
    Left,
    Right,
}

/// A minimap interaction delivered from the live `ControlBar.wnd:LeftHUD`.
///
/// Coordinates retain the actual WND rectangle rather than assuming a fixed
/// HUD layout.  Main stamps that rectangle into its WGPU minimap mapping
/// before converting the click to a world position and applying its FOW gate.
/// Input provenance is attached only while publishing from the synchronous
/// WND callback; it must never be inferred from a legacy `FromUser` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostMinimapInteraction {
    pub screen_position: [i32; 2],
    pub screen_top_left: [i32; 2],
    pub screen_size: [i32; 2],
    pub button: HostMinimapMouseButton,
    pub alternate_mouse: bool,
    pub input_provenance: HostControlBarInputProvenance,
}

/// Input owned by the LeftHUD callback before the bridge captures provenance.
///
/// Keeping this crate-private prevents a caller from constructing a published
/// interaction with a forged physical provenance value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostMinimapInteractionRequest {
    pub screen_position: [i32; 2],
    pub screen_top_left: [i32; 2],
    pub screen_size: [i32; 2],
    pub button: HostMinimapMouseButton,
    pub alternate_mouse: bool,
}

/// A gameplay request issued by a Control Bar interaction.
///
/// Names are preserved alongside the legacy [`CommandType`] so the Main host
/// can map directly to its typed Rust command system instead of guessing from
/// numeric ThingTemplate/upgrade IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostControlBarRequest {
    /// Select the next idle worker from either retail idle-worker UI control.
    ///
    /// C++ dispatches both `ControlBar.wnd:ButtonIdleWorker` and
    /// `IdleWorker.wnd:ButtonSelectNextIdleWorker` straight to
    /// `InGameUI::selectNextIdleWorker`.  Main owns the offline match world,
    /// so this must not enter the standalone GameLogic selection path.
    SelectNextIdleWorker,
    /// Cancel the active dozer/worker structure-placement mode.
    ///
    /// C++ clears the placement state when the Generals Experience panel gains
    /// the mouse.  Main owns that state for an offline host match, so this
    /// must not mutate the standalone GameLogic UI while the bridge is active.
    CancelStructurePlacement,
    /// Acknowledge the single active in-game script popup.
    ///
    /// The offline executable owns the authoritative popup residual and its
    /// pause lifecycle.  Keep the acknowledgement typed so the callback does
    /// not clear only GameClient's compatibility GameLogic while Main retains
    /// the visible popup and pause state.
    DismissInGamePopupMessage {
        /// Opaque Main-issued instance id; zero is never publishable. It
        /// prevents an acknowledgement queued for an older popup from
        /// dismissing the C++-style replacement currently on screen.
        popup_generation: usize,
    },
    /// Queue a unit or structure template at every selected producer.
    Production {
        command_name: String,
        template_name: String,
        producer_ids: Vec<u32>,
        player_id: u32,
        source: CommandSourceType,
    },
    /// Queue a named upgrade at the selected producer(s).
    Upgrade {
        command_name: String,
        upgrade_name: String,
        selected_object_ids: Vec<u32>,
        player_id: u32,
        source: CommandSourceType,
    },
    /// Execute a non-targeted named command, preserving its legacy identity.
    DirectCommand {
        command_name: String,
        command_type: CommandType,
        options: u32,
        /// Weapon slot (primary/secondary/tertiary) when the command is a
        /// legacy FIRE_WEAPON or SWITCH_WEAPON button.
        weapon_slot: Option<u32>,
        /// Exact parsed `MaxShotsToFire =` value for a legacy FIRE_WEAPON
        /// button.  It is absent for command kinds to which C++ does not
        /// attach a weapon-shot budget.
        max_shots_to_fire: Option<i32>,
        /// The raw CommandButton `Object=` identity when one was supplied.
        ///
        /// Only `QueueUnitCreate` treats this as a production template.  Other
        /// commands use it for selection, display, or special-power payloads.
        object_name: Option<String>,
        /// Required-upgrade metadata carried by the button, when any.
        upgrade_name: Option<String>,
        selected_object_ids: Vec<u32>,
        player_id: u32,
        source: CommandSourceType,
        /// Names supplied by a PurchaseScience-style command, if any.
        science_names: Vec<String>,
        /// Exact special-power identity for immediate/non-target powers.
        special_power_name: Option<String>,
        /// Legacy special-power ID when the GameLogic button bridge has it.
        special_power_id: Option<u32>,
        /// C++ MSG_EXIT occupant (`m_containData.objectID`), not the container.
        exit_object_id: Option<u32>,
    },
    /// Arm a placement/targeting interaction for the host input layer.
    ArmTarget {
        command_name: String,
        command_type: CommandType,
        options: u32,
        /// Fire-weapon slot (primary/secondary/tertiary) when this target arm
        /// came from a legacy FIRE_WEAPON button.
        weapon_slot: Option<u32>,
        /// Exact parsed `MaxShotsToFire =` value when this target arm came
        /// from a legacy FIRE_WEAPON button.
        max_shots_to_fire: Option<i32>,
        /// Raw CommandButton `Object=` identity, including a
        /// SPECIAL_POWER_CONSTRUCT payload such as SneakAttack's spawn unit.
        object_name: Option<String>,
        /// Required-upgrade metadata carried by the button, when any.
        upgrade_name: Option<String>,
        selected_object_ids: Vec<u32>,
        player_id: u32,
        source: CommandSourceType,
        /// The first selected object, matching the C++ pending-command source.
        source_object_id: Option<u32>,
        target: HostControlBarTarget,
    },
    /// Cancel the displayed production entry from a selected producer.
    QueueCancel {
        player_id: u32,
        selected_object_ids: Vec<u32>,
        producer_id: u32,
        production_id: u32,
        production_type: QueueProductionType,
        upgrade_name: String,
        queue_index: usize,
    },
    /// Pause or resume production for one selected producer.
    ProductionPause {
        player_id: u32,
        selected_object_ids: Vec<u32>,
        producer_id: u32,
        paused: bool,
    },
}

/// A request plus the input provenance captured at publication time.
///
/// Main must use this detailed form at its single-authority boundary.  The
/// legacy request-only drain is retained for standalone GameClient callers,
/// where provenance is intentionally irrelevant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostControlBarPublishedRequest {
    pub request: HostControlBarRequest,
    pub input_provenance: HostControlBarInputProvenance,
}

#[derive(Default)]
struct HostControlBarBridgeState {
    enabled: bool,
    requests: VecDeque<HostControlBarPublishedRequest>,
    minimap_interactions: VecDeque<HostMinimapInteraction>,
}

fn host_control_bar_bridge_state() -> &'static Mutex<HostControlBarBridgeState> {
    static STATE: OnceLock<Mutex<HostControlBarBridgeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(HostControlBarBridgeState::default()))
}

/// A safe, stack-scoped input context for synchronous WND dispatch.
///
/// This deliberately is not a thread-local pointer or a lifetime-erased
/// callback.  The context is process-global only so GameClient's existing
/// singleton callback path can read it, while each entry is bound to the
/// dispatching thread and removed by RAII before it can leak to later input.
#[derive(Debug, Clone, Copy)]
struct HostControlBarInputScopeEntry {
    id: u64,
    thread_id: std::thread::ThreadId,
    provenance: HostControlBarInputProvenance,
}

#[derive(Default)]
struct HostControlBarInputContextState {
    next_id: u64,
    scopes: Vec<HostControlBarInputScopeEntry>,
}

fn host_control_bar_input_context_state() -> &'static Mutex<HostControlBarInputContextState> {
    static STATE: OnceLock<Mutex<HostControlBarInputContextState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(HostControlBarInputContextState::default()))
}

struct HostControlBarInputScope {
    id: u64,
    thread_id: std::thread::ThreadId,
}

impl HostControlBarInputScope {
    fn enter(provenance: HostControlBarInputProvenance) -> Self {
        let thread_id = std::thread::current().id();
        let mut state = host_control_bar_input_context_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.scopes.push(HostControlBarInputScopeEntry {
            id,
            thread_id: thread_id.clone(),
            provenance,
        });
        Self { id, thread_id }
    }
}

impl Drop for HostControlBarInputScope {
    fn drop(&mut self) {
        let mut state = host_control_bar_input_context_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(index) = state
            .scopes
            .iter()
            .rposition(|entry| entry.id == self.id && entry.thread_id == self.thread_id)
        {
            state.scopes.remove(index);
        }
    }
}

/// Capture the current synchronous WND-dispatch provenance for an event that
/// will be processed later by the live Control Bar tick.
///
/// The WND callback first lands in `LiveControlBarEvents`; it does not publish
/// the host request until that deferred event is replayed.  Carrying this
/// value across that queue preserves the real physical/injected distinction
/// without extending the scope beyond its original OS dispatch.
pub(crate) fn host_control_bar_input_provenance_for_current_dispatch()
-> HostControlBarInputProvenance {
    let thread_id = std::thread::current().id();
    host_control_bar_input_context_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .scopes
        .iter()
        .rev()
        .find(|entry| entry.thread_id == thread_id)
        .map(|entry| entry.provenance)
        .unwrap_or(HostControlBarInputProvenance::InjectedOrUnknown)
}

/// Publish WND-driven work with explicit input provenance for this synchronous
/// dispatch only.
///
/// Main calls this around the exact WindowManager dispatch that originated
/// from `WindowEvent::MouseInput`; its injected path enters
/// `InjectedOrUnknown`.  The scope is thread-bound, nest-safe, and dropped
/// even when the callback unwinds, so a provenance mark cannot be reused by a
/// later request.
pub fn with_host_control_bar_input_provenance<R>(
    provenance: HostControlBarInputProvenance,
    callback: impl FnOnce() -> R,
) -> R {
    let _scope = HostControlBarInputScope::enter(provenance);
    callback()
}

/// Enable or disable authoritative-host delivery for Control Bar interactions.
///
/// It is disabled by default so isolated GameClient/GameLogic builds retain
/// their original command-queue behavior.  Changing modes clears pending work:
/// neither a typed request nor a legacy pause residual generated for a prior
/// owner can be replayed into a new one.
pub fn set_host_control_bar_bridge_enabled(enabled: bool) {
    let changed = {
        let mut state = host_control_bar_bridge_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.enabled != enabled {
            state.enabled = enabled;
            state.requests.clear();
            state.minimap_interactions.clear();
            true
        } else {
            false
        }
    };
    if changed {
        // This queue predates the typed bridge and is consumed by Main's old
        // compatibility drain.  Flush it after releasing the bridge mutex so
        // no old-mode pause can cross the single-authority boundary.
        super::control_bar::clear_host_production_pause_requests();
    }
}

/// Whether live Control Bar actions are being sent to the authoritative host.
pub fn host_control_bar_bridge_enabled() -> bool {
    host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .enabled
}

/// Drain typed requests together with the exact input provenance captured at
/// their publication point.
pub fn take_host_control_bar_published_requests() -> Vec<HostControlBarPublishedRequest> {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.requests.drain(..).collect()
}

/// Drain WND LeftHUD minimap interactions at Main's authoritative boundary.
///
/// Just like Control Bar requests, these are meaningful only while the host
/// bridge owns the live callback path.  The event retains its publication-time
/// provenance even though minimap gameplay itself does not make a physical
/// acceptance claim.
pub fn take_host_minimap_interactions() -> Vec<HostMinimapInteraction> {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.minimap_interactions.drain(..).collect()
}

/// Drain only request payloads for legacy standalone callers.
///
/// The Main executable must call [`take_host_control_bar_published_requests`]
/// instead so it cannot accidentally discard physical-input provenance.
pub fn take_host_control_bar_requests() -> Vec<HostControlBarRequest> {
    take_host_control_bar_published_requests()
        .into_iter()
        .map(|published| published.request)
        .collect()
}

/// Discard typed requests that have not yet been consumed by the host.
pub fn clear_host_control_bar_requests() {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.requests.clear();
    state.minimap_interactions.clear();
}

/// Discard only stale popup acknowledgements at an authoritative world
/// boundary.  Unlike [`clear_host_control_bar_requests`], this deliberately
/// preserves unrelated UI work: a reset/load invalidates an old popup WND,
/// not every Control Bar interaction already captured for the new world.
pub fn clear_host_dismiss_in_game_popup_message_requests() {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.requests.retain(|published| {
        !matches!(
            &published.request,
            HostControlBarRequest::DismissInGamePopupMessage { .. }
        )
    });
}

/// Build a host request from the exact data attached to a Control Bar button.
///
/// This is crate-visible so the live Control Bar can enrich special-power
/// targeting with the legacy template ID when available, while tests can
/// verify classification without initializing global GameLogic state.
pub(crate) fn host_request_from_button(
    button: &CommandButton,
    context: &ControlBarContext,
    source: CommandSourceType,
    special_power_id: Option<u32>,
    command_needs_target: bool,
) -> HostControlBarRequest {
    host_request_from_button_with_weapon_slot(
        button,
        context,
        source,
        special_power_id,
        None,
        command_needs_target,
    )
}

/// As [`host_request_from_button`], with a resolved FIRE_WEAPON or
/// SWITCH_WEAPON slot.
pub(crate) fn host_request_from_button_with_weapon_slot(
    button: &CommandButton,
    context: &ControlBarContext,
    source: CommandSourceType,
    special_power_id: Option<u32>,
    weapon_slot: Option<u32>,
    command_needs_target: bool,
) -> HostControlBarRequest {
    // Retail SPECIAL_POWER_CONSTRUCT buttons (notably SneakAttack) encode a
    // special power plus an Object= payload but omit NEED_TARGET_* options.
    // They still enter map-target mode in C++; keep that semantic explicit
    // instead of mistaking the object for unit production or an instant cast.
    let is_special_power_construct = !button.special_power.is_empty() && !button.object.is_empty();
    // `MaxShotsToFire` belongs specifically to FIRE_WEAPON.  Preserve the
    // parsed signed value verbatim: C++ uses `NO_MAX_SHOTS_LIMIT` as a real
    // value, and a zero budget must not be rewritten into an unlimited order.
    let max_shots_to_fire =
        (button.command_type == CommandType::FireWeapon).then_some(button.max_shots_to_fire);
    if command_needs_target || is_special_power_construct {
        let target =
            if button.command_type == CommandType::DozerConstruct && !button.object.is_empty() {
                HostControlBarTarget::DozerConstruct {
                    template_name: button.object.clone(),
                }
            } else if !button.special_power.is_empty() {
                HostControlBarTarget::SpecialPower {
                    special_power_name: button.special_power.clone(),
                    special_power_id,
                }
            } else {
                HostControlBarTarget::Generic
            };

        return HostControlBarRequest::ArmTarget {
            command_name: button.command_name.clone(),
            command_type: button.command_type,
            options: button.options,
            weapon_slot,
            max_shots_to_fire,
            object_name: (!button.object.is_empty()).then(|| button.object.clone()),
            upgrade_name: (!button.upgrade.is_empty()).then(|| button.upgrade.clone()),
            selected_object_ids: context.selected_objects.clone(),
            player_id: context.player_id,
            source,
            source_object_id: context.selected_objects.first().copied(),
            target,
        };
    }

    // An Upgrade= field also expresses a prerequisite on ordinary buttons
    // (FireWeapon, switch weapon, etc.).  C++ queues an upgrade only for the
    // actual QueueUpgrade command, so metadata must stay with the command.
    if button.command_type == CommandType::QueueUpgrade && !button.upgrade.is_empty() {
        return HostControlBarRequest::Upgrade {
            command_name: button.command_name.clone(),
            upgrade_name: button.upgrade.clone(),
            selected_object_ids: context.selected_objects.clone(),
            player_id: context.player_id,
            source,
        };
    }

    // These buttons can carry Object= as metadata, not as a unit to queue:
    // e.g. PurchaseSciencePaladin, SelectAllUnitsOfType, and
    // SpecialPowerConstruct/SneakAttack.  Preserve the object in the direct
    // request; only C++ MSG_QUEUE_UNIT_CREATE semantics are production.
    if !button.special_power.is_empty() || button.command_type == CommandType::PurchaseScience {
        return direct_host_request(
            button,
            context,
            source,
            special_power_id,
            weapon_slot,
            max_shots_to_fire,
        );
    }

    if button.command_type == CommandType::QueueUnitCreate && !button.object.is_empty() {
        return HostControlBarRequest::Production {
            command_name: button.command_name.clone(),
            template_name: button.object.clone(),
            producer_ids: context.selected_objects.clone(),
            player_id: context.player_id,
            source,
        };
    }

    direct_host_request(
        button,
        context,
        source,
        special_power_id,
        weapon_slot,
        max_shots_to_fire,
    )
}

fn direct_host_request(
    button: &CommandButton,
    context: &ControlBarContext,
    source: CommandSourceType,
    special_power_id: Option<u32>,
    weapon_slot: Option<u32>,
    max_shots_to_fire: Option<i32>,
) -> HostControlBarRequest {
    HostControlBarRequest::DirectCommand {
        command_name: button.command_name.clone(),
        command_type: button.command_type,
        options: button.options,
        weapon_slot,
        max_shots_to_fire,
        object_name: (!button.object.is_empty()).then(|| button.object.clone()),
        upgrade_name: (!button.upgrade.is_empty()).then(|| button.upgrade.clone()),
        selected_object_ids: context.selected_objects.clone(),
        player_id: context.player_id,
        source,
        science_names: button.sciences.clone(),
        special_power_name: (!button.special_power.is_empty())
            .then(|| button.special_power.clone()),
        special_power_id: if button.special_power.is_empty() {
            None
        } else {
            special_power_id
        },
        exit_object_id: button.exit_object_id,
    }
}

/// Publish a request only while the host bridge owns Control Bar authority.
///
/// Returning `false` lets normal GameClient callers fall through to the
/// original GameLogic/message-stream behavior unchanged.
pub(crate) fn publish_host_control_bar_request(request: HostControlBarRequest) -> bool {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.enabled {
        return false;
    }

    state.requests.push_back(HostControlBarPublishedRequest {
        request,
        input_provenance: host_control_bar_input_provenance_for_current_dispatch(),
    });
    true
}

/// Publish the shared retail idle-worker selection action while Main owns the
/// Control Bar.  Returning `false` deliberately lets standalone GameClient
/// callers keep their legacy selection/message-stream behavior.
pub(crate) fn publish_host_select_next_idle_worker() -> bool {
    publish_host_control_bar_request(HostControlBarRequest::SelectNextIdleWorker)
}

/// Publish the retail Generals Experience placement-cancel action while Main
/// owns the Control Bar. Returning `false` deliberately preserves the legacy
/// `TheInGameUI::place_build_available(None, None)` fallback for standalone
/// GameClient callers.
pub(crate) fn publish_host_cancel_structure_placement() -> bool {
    publish_host_control_bar_request(HostControlBarRequest::CancelStructurePlacement)
}

/// Publish acknowledgement of the active script popup while Main owns the
/// offline UI authority. Returning `false` deliberately preserves the C++
/// GameClient direct/message-stream fallback for standalone callers.
pub(crate) fn publish_host_dismiss_in_game_popup_message(popup_generation: usize) -> bool {
    if popup_generation == 0 {
        return false;
    }
    publish_host_control_bar_request(HostControlBarRequest::DismissInGamePopupMessage {
        popup_generation,
    })
}

/// Publish a LeftHUD minimap click only while Main owns Control Bar authority.
///
/// The callback supplies geometry and button semantics, but this function
/// captures the scoped WND provenance itself.  That keeps both physical OS
/// events and injected/runtime-host events on the same gameplay route without
/// allowing the latter to impersonate a physical click.
pub(crate) fn publish_host_minimap_interaction(request: HostMinimapInteractionRequest) -> bool {
    let mut state = host_control_bar_bridge_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.enabled {
        return false;
    }

    state
        .minimap_interactions
        .push_back(HostMinimapInteraction {
            screen_position: request.screen_position,
            screen_top_left: request.screen_top_left,
            screen_size: request.screen_size,
            button: request.button,
            alternate_mouse: request.alternate_mouse,
            input_provenance: host_control_bar_input_provenance_for_current_dispatch(),
        });
    true
}

/// Publish a host-only queue cancellation before any legacy side effects run.
pub(crate) fn publish_host_queue_cancel(
    context: &ControlBarContext,
    producer_id: u32,
    production_id: u32,
    production_type: QueueProductionType,
    upgrade_name: String,
    queue_index: usize,
) -> bool {
    publish_host_control_bar_request(HostControlBarRequest::QueueCancel {
        player_id: context.player_id,
        selected_object_ids: context.selected_objects.clone(),
        producer_id,
        production_id,
        production_type,
        upgrade_name,
        queue_index,
    })
}

/// Publish a host-only production-pause change before legacy module updates.
pub(crate) fn publish_host_production_pause(
    context: &ControlBarContext,
    producer_id: u32,
    paused: bool,
) -> bool {
    publish_host_control_bar_request(HostControlBarRequest::ProductionPause {
        player_id: context.player_id,
        selected_object_ids: context.selected_objects.clone(),
        producer_id,
        paused,
    })
}

#[cfg(test)]
static HOST_CONTROL_BAR_BRIDGE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serialize unit tests that alter the process-global host bridge state.
#[cfg(test)]
pub(crate) struct HostControlBarBridgeTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
pub(crate) fn acquire_host_control_bar_bridge_test_guard() -> HostControlBarBridgeTestGuard {
    let lock = HOST_CONTROL_BAR_BRIDGE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_host_control_bar_requests();
    set_host_control_bar_bridge_enabled(false);
    HostControlBarBridgeTestGuard { _lock: lock }
}

#[cfg(test)]
impl Drop for HostControlBarBridgeTestGuard {
    fn drop(&mut self) {
        clear_host_control_bar_requests();
        set_host_control_bar_bridge_enabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acquire() -> HostControlBarBridgeTestGuard {
        acquire_host_control_bar_bridge_test_guard()
    }

    fn context() -> ControlBarContext {
        ControlBarContext {
            player_id: 7,
            selected_objects: vec![41, 42],
            ..ControlBarContext::default()
        }
    }

    #[test]
    fn bridge_is_opt_in_and_drains_typed_production_requests() {
        let _guard = acquire();
        let mut button = CommandButton::default();
        button.command_name = "Command_ConstructAmericaTank".to_string();
        button.command_type = CommandType::QueueUnitCreate;
        button.object = "AmericaTankCrusader".to_string();

        let request = host_request_from_button(
            &button,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        );
        assert!(
            !publish_host_control_bar_request(request.clone()),
            "disabled bridge must leave standalone GameClient on its legacy path"
        );
        assert!(take_host_control_bar_requests().is_empty());

        set_host_control_bar_bridge_enabled(true);
        assert!(publish_host_control_bar_request(request));
        assert_eq!(
            take_host_control_bar_requests(),
            vec![HostControlBarRequest::Production {
                command_name: "Command_ConstructAmericaTank".to_string(),
                template_name: "AmericaTankCrusader".to_string(),
                producer_ids: vec![41, 42],
                player_id: 7,
                source: CommandSourceType::FromUser,
            }]
        );
    }

    #[test]
    fn bridge_routes_idle_worker_selection_only_when_enabled() {
        let _guard = acquire();
        assert!(
            !publish_host_select_next_idle_worker(),
            "standalone GameClient must retain its legacy idle-worker route"
        );

        set_host_control_bar_bridge_enabled(true);
        assert!(publish_host_select_next_idle_worker());
        assert!(matches!(
            take_host_control_bar_requests().as_slice(),
            [HostControlBarRequest::SelectNextIdleWorker]
        ));
    }

    #[test]
    fn bridge_routes_structure_placement_cancel_only_when_enabled() {
        let _guard = acquire();
        assert!(
            !publish_host_cancel_structure_placement(),
            "standalone GameClient must retain its legacy placement-cancel route"
        );

        set_host_control_bar_bridge_enabled(true);
        assert!(publish_host_cancel_structure_placement());
        assert!(matches!(
            take_host_control_bar_requests().as_slice(),
            [HostControlBarRequest::CancelStructurePlacement]
        ));
    }

    #[test]
    fn bridge_routes_popup_acknowledgement_only_when_enabled() {
        let _guard = acquire();
        assert!(
            !publish_host_dismiss_in_game_popup_message(17),
            "standalone GameClient must retain its legacy popup-dismiss route"
        );

        set_host_control_bar_bridge_enabled(true);
        assert!(publish_host_dismiss_in_game_popup_message(17));
        assert!(matches!(
            take_host_control_bar_requests().as_slice(),
            [HostControlBarRequest::DismissInGamePopupMessage {
                popup_generation: 17
            }]
        ));

        assert!(
            !publish_host_dismiss_in_game_popup_message(0),
            "zero means no Main-owned popup identity and must keep legacy fallback"
        );
        assert!(take_host_control_bar_requests().is_empty());
    }

    #[test]
    fn popup_boundary_clear_discards_only_popup_acknowledgements() {
        let _guard = acquire();
        set_host_control_bar_bridge_enabled(true);
        assert!(publish_host_dismiss_in_game_popup_message(23));
        assert!(publish_host_select_next_idle_worker());

        clear_host_dismiss_in_game_popup_message_requests();
        assert!(matches!(
            take_host_control_bar_requests().as_slice(),
            [HostControlBarRequest::SelectNextIdleWorker]
        ));
    }

    #[test]
    fn publication_captures_scoped_physical_provenance_and_fails_closed_otherwise() {
        let _guard = acquire();
        set_host_control_bar_bridge_enabled(true);

        let request = HostControlBarRequest::Production {
            command_name: "Command_ConstructAmericaTank".to_string(),
            template_name: "AmericaTankCrusader".to_string(),
            producer_ids: vec![41],
            player_id: 7,
            source: CommandSourceType::FromUser,
        };

        // `FromUser` without Main's physical WND scope must never imply a
        // physical mouse click. This covers direct/test/runtime-host paths.
        assert!(publish_host_control_bar_request(request.clone()));
        with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::InjectedOrUnknown,
            || assert!(publish_host_control_bar_request(request.clone())),
        );
        with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::PhysicalWindowMouseInput,
            || assert!(publish_host_control_bar_request(request)),
        );

        let published = take_host_control_bar_published_requests();
        assert_eq!(published.len(), 3);
        assert_eq!(
            published[0].input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown,
            "unscoped FromUser request must fail closed"
        );
        assert_eq!(
            published[1].input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown,
            "injected scope must remain non-physical"
        );
        assert_eq!(
            published[2].input_provenance,
            HostControlBarInputProvenance::PhysicalWindowMouseInput,
            "only Main's real WND mouse scope may mark a request physical"
        );

        // Scope cleanup is RAII: the next request must not inherit physical
        // provenance after the synchronous callback returns.
        assert!(publish_host_control_bar_request(
            HostControlBarRequest::Production {
                command_name: "Command_ConstructAmericaTank".to_string(),
                template_name: "AmericaTankCrusader".to_string(),
                producer_ids: vec![41],
                player_id: 7,
                source: CommandSourceType::FromUser,
            }
        ));
        assert_eq!(
            take_host_control_bar_published_requests()[0].input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown,
            "a finished physical scope must not leak to a later request"
        );
    }

    #[test]
    fn minimap_publication_captures_wnd_provenance_without_forging_it() {
        let _guard = acquire();
        let request = HostMinimapInteractionRequest {
            screen_position: [42, 491],
            screen_top_left: [7, 443],
            screen_size: [167, 152],
            button: HostMinimapMouseButton::Right,
            alternate_mouse: false,
        };

        assert!(
            !publish_host_minimap_interaction(request),
            "a disabled bridge must leave the standalone radar callback alone"
        );
        assert!(take_host_minimap_interactions().is_empty());

        set_host_control_bar_bridge_enabled(true);
        // Neither a direct callback/test invocation nor an injected WND scope
        // may claim to be a physical OS mouse event.
        assert!(publish_host_minimap_interaction(request));
        with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::InjectedOrUnknown,
            || assert!(publish_host_minimap_interaction(request)),
        );
        with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::PhysicalWindowMouseInput,
            || assert!(publish_host_minimap_interaction(request)),
        );

        let published = take_host_minimap_interactions();
        assert_eq!(published.len(), 3);
        assert_eq!(
            published[0].input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown
        );
        assert_eq!(
            published[1].input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown
        );
        assert_eq!(
            published[2].input_provenance,
            HostControlBarInputProvenance::PhysicalWindowMouseInput
        );
        assert_eq!(published[2].screen_top_left, [7, 443]);
        assert_eq!(published[2].screen_size, [167, 152]);
        assert_eq!(published[2].button, HostMinimapMouseButton::Right);
    }

    #[test]
    fn bridge_mode_change_discards_stale_legacy_pause_requests() {
        let _guard = acquire();
        super::super::control_bar::queue_host_production_pause(41, true);
        set_host_control_bar_bridge_enabled(true);
        assert!(
            super::super::control_bar::take_host_production_pause_requests().is_empty(),
            "a pause from the legacy owner must not reach a newly enabled host bridge"
        );
    }

    #[test]
    fn target_upgrade_direct_cancel_and_pause_keep_host_meaning() {
        let _guard = acquire();
        set_host_control_bar_bridge_enabled(true);

        let mut dozer = CommandButton::default();
        dozer.command_name = "Command_ConstructAmericaPowerPlant".to_string();
        dozer.command_type = CommandType::DozerConstruct;
        dozer.object = "AmericaPowerPlant".to_string();
        dozer.options = 0x20;
        let arm =
            host_request_from_button(&dozer, &context(), CommandSourceType::FromUser, None, true);
        assert!(publish_host_control_bar_request(arm));

        let mut upgrade = CommandButton::default();
        upgrade.command_name = "Command_UpgradeCompositeArmor".to_string();
        upgrade.command_type = CommandType::QueueUpgrade;
        upgrade.upgrade = "Upgrade_AmericaCompositeArmor".to_string();
        assert!(publish_host_control_bar_request(host_request_from_button(
            &upgrade,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        )));

        let mut direct = CommandButton::default();
        direct.command_name = "Command_Sell".to_string();
        direct.command_type = CommandType::Sell;
        assert!(publish_host_control_bar_request(host_request_from_button(
            &direct,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        )));
        assert!(publish_host_queue_cancel(
            &context(),
            41,
            99,
            QueueProductionType::Upgrade,
            "Upgrade_AmericaCompositeArmor".to_string(),
            0,
        ));
        assert!(publish_host_production_pause(&context(), 41, true));

        let requests = take_host_control_bar_requests();
        assert!(matches!(
            &requests[0],
            HostControlBarRequest::ArmTarget {
                command_name,
                source_object_id: Some(41),
                target: HostControlBarTarget::DozerConstruct { template_name },
                ..
            } if command_name == "Command_ConstructAmericaPowerPlant"
                && template_name == "AmericaPowerPlant"
        ));
        assert!(matches!(
            &requests[1],
            HostControlBarRequest::Upgrade { upgrade_name, .. }
                if upgrade_name == "Upgrade_AmericaCompositeArmor"
        ));
        assert!(matches!(
            &requests[2],
            HostControlBarRequest::DirectCommand {
                command_name,
                command_type: CommandType::Sell,
                ..
            } if command_name == "Command_Sell"
        ));
        assert!(matches!(
            &requests[3],
            HostControlBarRequest::QueueCancel {
                producer_id: 41,
                production_id: 99,
                production_type: QueueProductionType::Upgrade,
                ..
            }
        ));
        assert!(matches!(
            &requests[4],
            HostControlBarRequest::ProductionPause {
                producer_id: 41,
                paused: true,
                ..
            }
        ));
    }

    #[test]
    fn purchase_science_with_object_display_field_is_never_production() {
        let _guard = acquire();
        let mut button = CommandButton::default();
        button.command_name = "Command_PurchaseSciencePaladin".to_string();
        button.command_type = CommandType::PurchaseScience;
        // Retail CommandButton definitions overload Object= for this kind of
        // science cameo.  It must not become a production template request.
        button.object = "AmericaTankPaladin".to_string();
        button.sciences = vec!["SCIENCE_PaladinTank".to_string()];

        let request = host_request_from_button(
            &button,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        );
        assert!(matches!(
            request,
            HostControlBarRequest::DirectCommand {
                command_name,
                command_type: CommandType::PurchaseScience,
                object_name: Some(object_name),
                science_names,
                ..
            } if command_name == "Command_PurchaseSciencePaladin"
                && object_name == "AmericaTankPaladin"
                && science_names == ["SCIENCE_PaladinTank"]
        ));
    }

    #[test]
    fn object_bearing_non_production_commands_keep_their_real_semantics() {
        let _guard = acquire();

        let mut select_all = CommandButton::default();
        select_all.command_name = "Command_SelectAllPaladins".to_string();
        select_all.command_type = CommandType::MetaSelectMatchingUnits;
        select_all.object = "AmericaTankPaladin".to_string();
        let request = host_request_from_button(
            &select_all,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        );
        assert!(matches!(
            request,
            HostControlBarRequest::DirectCommand {
                command_type: CommandType::MetaSelectMatchingUnits,
                object_name: Some(object_name),
                ..
            } if object_name == "AmericaTankPaladin"
        ));

        let mut sneak_attack = CommandButton::default();
        sneak_attack.command_name = "Command_SneakAttack".to_string();
        sneak_attack.command_type = CommandType::DoSpecialPower;
        sneak_attack.object = "GLAInfantryRebel".to_string();
        sneak_attack.special_power = "SuperweaponSneakAttack".to_string();
        let request = host_request_from_button(
            &sneak_attack,
            &context(),
            CommandSourceType::FromUser,
            Some(99),
            false,
        );
        assert!(matches!(
            request,
            HostControlBarRequest::ArmTarget {
                object_name: Some(object_name),
                target: HostControlBarTarget::SpecialPower {
                    special_power_name,
                    special_power_id: Some(99),
                },
                ..
            } if object_name == "GLAInfantryRebel"
                && special_power_name == "SuperweaponSneakAttack"
        ));
    }

    #[test]
    fn required_upgrade_metadata_does_not_turn_an_action_into_queue_upgrade() {
        let _guard = acquire();

        let mut fire_weapon = CommandButton::default();
        fire_weapon.command_name = "Command_ComancheFireRocketPods".to_string();
        fire_weapon.command_type = CommandType::FireWeapon;
        fire_weapon.upgrade = "Upgrade_AmericaComancheRocketPods".to_string();
        fire_weapon.options = 0x01;
        let armed = host_request_from_button(
            &fire_weapon,
            &context(),
            CommandSourceType::FromUser,
            None,
            true,
        );
        assert!(matches!(
            armed,
            HostControlBarRequest::ArmTarget {
                command_type: CommandType::FireWeapon,
                upgrade_name: Some(upgrade_name),
                ..
            } if upgrade_name == "Upgrade_AmericaComancheRocketPods"
        ));

        let mut switch_weapon = CommandButton::default();
        switch_weapon.command_name = "Command_AmericaRangerSwitchToFlagBangGrenades".to_string();
        switch_weapon.command_type = CommandType::SwitchWeapons;
        switch_weapon.upgrade = "Upgrade_AmericaRangerFlashBangGrenades".to_string();
        let direct = host_request_from_button(
            &switch_weapon,
            &context(),
            CommandSourceType::FromUser,
            None,
            false,
        );
        assert!(matches!(
            direct,
            HostControlBarRequest::DirectCommand {
                command_type: CommandType::SwitchWeapons,
                upgrade_name: Some(upgrade_name),
                ..
            } if upgrade_name == "Upgrade_AmericaRangerFlashBangGrenades"
        ));
    }

    #[test]
    fn fire_weapon_slot_is_preserved_for_target_and_direct_requests() {
        let _guard = acquire();
        let mut fire_weapon = CommandButton::default();
        fire_weapon.command_name = "Command_ComancheFireTertiaryWeapon".to_string();
        fire_weapon.command_type = CommandType::FireWeapon;
        fire_weapon.max_shots_to_fire = 1;

        let armed = host_request_from_button_with_weapon_slot(
            &fire_weapon,
            &context(),
            CommandSourceType::FromUser,
            None,
            Some(2),
            true,
        );
        assert!(matches!(
            armed,
            HostControlBarRequest::ArmTarget {
                command_type: CommandType::FireWeapon,
                weapon_slot: Some(2),
                max_shots_to_fire: Some(1),
                target: HostControlBarTarget::Generic,
                ..
            }
        ));

        let direct = host_request_from_button_with_weapon_slot(
            &fire_weapon,
            &context(),
            CommandSourceType::FromUser,
            None,
            Some(1),
            false,
        );
        assert!(matches!(
            direct,
            HostControlBarRequest::DirectCommand {
                command_type: CommandType::FireWeapon,
                weapon_slot: Some(1),
                max_shots_to_fire: Some(1),
                ..
            }
        ));
    }

    #[test]
    fn special_and_generic_target_arms_preserve_identity_and_options() {
        let _guard = acquire();

        let mut special = CommandButton::default();
        special.command_name = "Command_A10Strike".to_string();
        special.command_type = CommandType::SpecialPower;
        special.special_power = "SuperweaponA10ThunderboltMissileStrike".to_string();
        special.options = 0x21;
        let special_request = host_request_from_button(
            &special,
            &context(),
            CommandSourceType::FromUser,
            Some(123),
            true,
        );
        assert!(matches!(
            special_request,
            HostControlBarRequest::ArmTarget {
                options: 0x21,
                target: HostControlBarTarget::SpecialPower {
                    special_power_name,
                    special_power_id: Some(123),
                },
                ..
            } if special_power_name == "SuperweaponA10ThunderboltMissileStrike"
        ));

        let immediate_special = host_request_from_button(
            &special,
            &context(),
            CommandSourceType::FromUser,
            Some(123),
            false,
        );
        assert!(matches!(
            immediate_special,
            HostControlBarRequest::DirectCommand {
                options: 0x21,
                special_power_name: Some(special_power_name),
                special_power_id: Some(123),
                ..
            } if special_power_name == "SuperweaponA10ThunderboltMissileStrike"
        ));

        let mut generic = CommandButton::default();
        generic.command_name = "Command_AttackMove".to_string();
        generic.command_type = CommandType::DoAttackMoveTo;
        generic.options = 0x1020;
        let generic_request =
            host_request_from_button(&generic, &context(), CommandSourceType::FromAI, None, true);
        assert!(matches!(
            generic_request,
            HostControlBarRequest::ArmTarget {
                command_type: CommandType::DoAttackMoveTo,
                options: 0x1020,
                source: CommandSourceType::FromAI,
                target: HostControlBarTarget::Generic,
                ..
            }
        ));
    }
}
