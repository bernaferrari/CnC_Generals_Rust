// Drawable IDs, presentation sync residual, and host flash queue.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Wave 963: presentation → drawable sync residual (host path, no dual-world registry).
#[derive(Debug, Clone)]
pub struct PresentationDrawableSync {
    pub object_id: u32,
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
    pub effectively_stealthed: bool,
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
