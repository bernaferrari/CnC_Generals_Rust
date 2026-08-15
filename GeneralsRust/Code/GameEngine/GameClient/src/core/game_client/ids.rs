// Drawable IDs, presentation sync residual, and host flash queue.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Runtime identity of one direct host-object visual binding.
///
/// This is deliberately client-local state: it identifies a currently live
/// `Drawable`, not a GameLogic object relationship that belongs in Xfer.  A
/// new world, replacement visual template, or recreated Drawable receives a
/// distinct key even if GameLogic reuses the same object id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationDirectDrawableBindingKey {
    /// Main's monotonically changing host-world identity.
    pub host_epoch: u64,
    /// GameLogic object identity within `host_epoch`.
    pub object_id: ObjectID,
    /// Runtime-only GameClient Drawable identity.
    pub drawable_id: DrawableId,
    /// Monotonic lifetime identity for this particular binding instance.
    pub binding_generation: u64,
}

/// Guarded direct-drawable shroud state exported to Main's render sidecar.
///
/// Callers must retain and compare the full [`PresentationDirectDrawableBindingKey`]
/// rather than treating a reused `ObjectID` as a stable visual owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationDirectDrawableState {
    pub binding_key: PresentationDirectDrawableBindingKey,
    /// Exact C++ `Drawable::isDrawableEffectivelyHidden` scene predicate.
    /// Main consumes this only in its frozen direct-host `Visibility_Check`
    /// sidecar; it is not the broader presentation `visible` flag.
    pub scene_effectively_hidden: bool,
    pub fully_obscured: bool,
}

/// Frozen world pose for an already-resolved direct visual binding.
///
/// The full key prevents a pose captured before a visual replacement from
/// moving the replacement Drawable by accident.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrozenDirectPresentationPose {
    pub binding_key: PresentationDirectDrawableBindingKey,
    pub position: [f32; 3],
    pub orientation: f32,
}

/// Private runtime metadata for a presentation-owned direct drawable.
///
/// It is intentionally absent from GameClient Xfer/snapshot data.  The
/// visual-template identity tells synchronization whether C++ would retain the
/// same Drawable or replace it, which in turn controls volatile direct shroud
/// state lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PresentationDirectDrawableBinding {
    binding_key: PresentationDirectDrawableBindingKey,
    visual_template_name: String,
}

/// Frozen direct-object shroud input for the GameClient visibility pass.
///
/// This deliberately carries the unmodified GameLogic
/// `ObjectShroudStatus`: C++ applies its clear-frame grace inside
/// `GameClient::update`, after obtaining the raw status for the currently
/// bound object.  Objectless drawables and ghosts use distinct W3D scene
/// branches and must not enter this batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectShroudStatus {
    /// Exact direct visual lifetime that produced this raw shroud status.
    /// The status is ignored unless all four identity components still match.
    pub binding_key: PresentationDirectDrawableBindingKey,
    /// Exact raw status returned by `Object::getShroudedStatus`.
    pub raw_status: gamelogic::common::types::ObjectShroudStatus,
    /// Exact `Object::isEffectivelyDead()` result for the extended grace limit.
    pub effectively_dead: bool,
}

/// One frozen direct visual that reached Main's W3D-equivalent scene
/// candidate boundary for this presentation frame.
///
/// Unlike [`FrozenDirectShroudStatus`], this input is not part of the
/// GameClient update visibility pass. Main produces it only after frozen
/// frustum acceptance, the current fully-obscured cull, and a real render
/// item have all succeeded. The consumer is therefore the sole permitted
/// direct-scene clear-frame writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectSceneShroudCandidate {
    /// Exact runtime binding that produced the candidate.
    pub binding_key: PresentationDirectDrawableBindingKey,
    /// Exact raw status for the frozen host object.
    pub raw_status: gamelogic::common::types::ObjectShroudStatus,
    /// Exact effective-death fact for the source grace extension.
    pub effectively_dead: bool,
}

/// One validated direct-scene outcome returned for a frozen candidate.
///
/// Main does not yet use this to route the future projected-shroud material
/// pass; retaining the decision keeps that pass keyed to the exact C++ scene
/// status rather than a scalar FOW approximation when it is wired later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectSceneShroudDecision {
    pub binding_key: PresentationDirectDrawableBindingKey,
    pub decision: crate::drawable::SceneShroudDecision,
}

/// Wave 963: presentation → drawable sync residual (host path, no dual-world registry).
#[derive(Debug, Clone)]
pub struct PresentationDrawableSync {
    pub object_id: u32,
    /// Runtime host-world identity supplied by Main for this direct visual.
    pub host_epoch: u64,
    /// Whether the host still owns this visual resident.  This is the only
    /// lifetime input used by `sync_presentation_drawables`; gameplay death is
    /// represented independently by `destroyed` and must not prune an active
    /// slow-death/rubble visual.
    pub resident: bool,
    /// Exact visual identity, including an active visual disguise.  A change
    /// replaces the runtime Drawable and resets non-Xfer volatile visual state.
    pub visual_template_name: String,
    pub template_name: String,
    pub position: [f32; 3],
    pub orientation: f32,
    pub destroyed: bool,
    pub model_condition_bits: u128,
    pub body_damage_state: u8,
    /// Wave 965: presentation KindOf Debug names (host empty dual-world kind queries).
    pub kind_names: Vec<String>,
    /// Wave 965: team tint residual 0..1 RGBA → indicator RGB.
    pub team_color: [f32; 4],
    /// Generic frozen host stealth state used by UI/overlay residuals.
    pub effectively_stealthed: bool,
    /// Exact viewer-relative C++ `m_hiddenByStealth` result for the direct
    /// scene path. This is intentionally separate from generic effective
    /// stealth: friendly stealthed units remain visible/translucent.
    pub scene_hidden_by_stealth: bool,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    /// Wave 970: overlay residual for host empty dual-world icon UI.
    pub veterancy_level: u8,
    pub under_construction: bool,
    pub construction_percent: f32,
    /// Wave 1115: C++ OBJECT_STATUS_SOLD residual for construct-percent fail-closed.
    pub sold: bool,
    /// Wave 972: icon-pip residual (ammo/contain/status).
    pub ammo_pip_total: u8,
    pub ammo_pip_full: u8,
    pub occupant_count: u8,
    pub max_garrison: u8,
    pub disabled: bool,
    pub is_carbomb: bool,
    pub weapon_bonus_enthusiastic: bool,
    /// Wave 983: healing icon residual for host empty dual-world.
    pub show_healing: bool,
    pub healing_icon_type: u8,
    /// Wave 984: garrisoned unit ids for contained-flash residual.
    pub garrisoned_ids: Vec<u32>,
    /// Wave 1057: emoticon residual name for dual icon UI.
    pub emoticon_name: String,
    /// Wave 1057: remaining frames for emoticon residual.
    pub emoticon_frames_left: i32,
    /// Wave 1058: formation id residual (0 = none) for dual group/formation letters.
    pub formation_id: u32,
    /// Wave 1059: unit caption residual (beacon/script caption) for dual draw_ui_text.
    pub caption: String,
}

// Wave 269: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Wave 984: host residual queue — flash contained presentation drawables on select.
static HOST_CONTAINED_FLASH_QUEUE: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Queue object ids whose presentation drawables should flash-as-selected.
pub fn queue_host_contained_flash_object_ids(ids: impl IntoIterator<Item = u32>) {
    if let Ok(mut q) = HOST_CONTAINED_FLASH_QUEUE.lock() {
        for id in ids {
            if id != 0 && !q.contains(&id) {
                q.push(id);
            }
        }
    }
}

fn take_host_contained_flash_object_ids() -> Vec<u32> {
    HOST_CONTAINED_FLASH_QUEUE
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// Result type for GameClient operations
pub type GameClientResult<T> = Result<T, GameClientError>;

/// Unique identifier for drawable objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrawableId(pub u32);

impl DrawableId {
    pub const INVALID: Self = DrawableId(0);

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}
