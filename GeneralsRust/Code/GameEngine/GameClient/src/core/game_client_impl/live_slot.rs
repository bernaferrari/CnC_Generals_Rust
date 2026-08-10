// Live GameClient slot, drawable-id counter, and LOD texture reduction.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

static GLOBAL_NEXT_DRAWABLE_ID: AtomicU32 = AtomicU32::new(1);
static LIVE_GAME_CLIENT_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();
const TEXTURE_REDUCTION_MIN: i32 = 0;
const TEXTURE_REDUCTION_MAX: i32 = 4;
const WW3D_TEXTURE_REDUCTION_MIN_DIMENSION: u32 = 32;

fn live_game_client_slot() -> &'static Mutex<Option<usize>> {
    LIVE_GAME_CLIENT_PTR.get_or_init(|| Mutex::new(None))
}

pub(crate) fn register_live_game_client(client: &mut GameClient) {
    if let Ok(mut slot) = live_game_client_slot().lock() {
        *slot = Some(client as *mut GameClient as usize);
    }
}

pub(crate) fn clear_live_game_client(client: &mut GameClient) {
    if let Ok(mut slot) = live_game_client_slot().lock() {
        let current = client as *mut GameClient as usize;
        if slot.is_some_and(|stored| stored == current) {
            *slot = None;
        }
    }
}

pub(crate) fn with_live_game_client_mut<R>(
    callback: impl FnOnce(&mut GameClient) -> R,
) -> Option<R> {
    let raw = live_game_client_slot().lock().ok().and_then(|slot| *slot)?;
    // The pointer is set from GameClient::init and cleared in Drop. Snapshot operations
    // occur while the owning GameClient is alive on the main thread.
    let client = unsafe { (raw as *mut GameClient).as_mut()? };
    Some(callback(client))
}

/// Apply a texture-reduction target immediately, mirroring C++ `W3DGameClient::adjustLOD`.
///
/// C++ reference:
/// - `W3DGameClient.cpp` `W3DGameClient::adjustLOD` (clamp [0,4], `WW3D::Set_Texture_Reduction`,
///   `TheTerrainRenderObject->setTextureLOD`)
pub fn apply_lod_texture_reduction(target_factor: i32) -> Option<i32> {
    let global_data = get_global_data()?;
    let clamped = target_factor.clamp(TEXTURE_REDUCTION_MIN, TEXTURE_REDUCTION_MAX);
    let previous = {
        let mut global = global_data.write();
        let previous = global
            .texture_reduction_factor
            .clamp(TEXTURE_REDUCTION_MIN, TEXTURE_REDUCTION_MAX);
        global.texture_reduction_factor = clamped;
        previous
    };

    if let Ok(mut runtime_global) = runtime_global_data::write_safe() {
        runtime_global.texture_reduction_factor = clamped;
    }

    let renderer_previous = ww3d_renderer_3d::rendering::texture_quality::texture_reduction();
    if renderer_previous != clamped as u32 {
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(
            clamped as u32,
            WW3D_TEXTURE_REDUCTION_MIN_DIMENSION,
        );
    }

    if previous != clamped || renderer_previous != clamped as u32 {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.apply_texture_lod_reduction(clamped);
            }
        }
    }

    Some(clamped)
}

/// Delta-based texture reduction adjustment wrapper (C++ `adjustLOD(adj)` semantics).
pub fn adjust_lod_texture_reduction(delta: i32) -> Option<i32> {
    let global_data = get_global_data()?;
    let current = {
        let global = global_data.read();
        global.texture_reduction_factor
    };
    apply_lod_texture_reduction(current.saturating_add(delta))
}
