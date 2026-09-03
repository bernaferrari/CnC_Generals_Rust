// Live GameClient slot, drawable-id counter, and LOD texture reduction.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

static GLOBAL_NEXT_DRAWABLE_ID: AtomicU32 = AtomicU32::new(1);
/// Published frame for the active client.  Input translators only need this
/// scalar; borrowing the whole client just to read it would alias the normal
/// `GameClient::update(&mut self)` path.
static LIVE_GAME_CLIENT_FRAME: AtomicU32 = AtomicU32::new(0);

/// Snapshot-only address of the engine-owned client.
///
/// `GameClient` contains window/UI state and cannot be owned by a global mutex.
/// The C++ save block nevertheless needs to reach the live instance.  Keep that
/// narrow compatibility bridge same-thread and dynamically exclusive: normal
/// input/render code must use published scalar state or its lexical `&mut
/// GameClient`, never this slot.
#[derive(Clone, Copy, Default)]
struct LiveGameClientSlot {
    raw: Option<usize>,
    owner_thread: Option<std::thread::ThreadId>,
    generation: u64,
    borrowed: bool,
}

static LIVE_GAME_CLIENT_SLOT: OnceLock<Mutex<LiveGameClientSlot>> = OnceLock::new();
const TEXTURE_REDUCTION_MIN: i32 = 0;
const TEXTURE_REDUCTION_MAX: i32 = 4;
const WW3D_TEXTURE_REDUCTION_MIN_DIMENSION: u32 = 32;

fn live_game_client_slot() -> &'static Mutex<LiveGameClientSlot> {
    LIVE_GAME_CLIENT_SLOT.get_or_init(|| Mutex::new(LiveGameClientSlot::default()))
}

pub(crate) fn register_live_game_client(client: &mut GameClient) {
    let Ok(mut slot) = live_game_client_slot().lock() else {
        return;
    };
    if slot.borrowed {
        // Replacing an address while its snapshot callback owns `&mut
        // GameClient` would invalidate the callback's exclusivity.  This is
        // not a valid lifecycle transition, so leave the existing slot live.
        log::error!("refusing to replace a borrowed live GameClient snapshot slot");
        return;
    }

    slot.generation = slot.generation.wrapping_add(1).max(1);
    slot.raw = Some(std::ptr::from_mut(client) as usize);
    slot.owner_thread = Some(std::thread::current().id());
    LIVE_GAME_CLIENT_FRAME.store(client.frame, Ordering::Release);
}

pub(crate) fn clear_live_game_client(client: &mut GameClient) {
    if let Ok(mut slot) = live_game_client_slot().lock() {
        let current = std::ptr::from_mut(client) as usize;
        if slot.raw.is_some_and(|stored| stored == current) {
            slot.raw = None;
            slot.owner_thread = None;
            slot.borrowed = false;
            LIVE_GAME_CLIENT_FRAME.store(0, Ordering::Release);
        }
    }
}

/// Refresh the scalar frame observable for input translators without borrowing
/// the live singleton.  A non-registered test/local client intentionally does
/// not replace the active engine's value.
pub(crate) fn publish_live_game_client_frame(client: &GameClient) {
    let current = std::ptr::from_ref(client) as usize;
    let current_thread = std::thread::current().id();
    let is_live = live_game_client_slot()
        .lock()
        .ok()
        .is_some_and(|slot| slot.raw == Some(current) && slot.owner_thread == Some(current_thread));
    if is_live {
        LIVE_GAME_CLIENT_FRAME.store(client.frame, Ordering::Release);
    }
}

/// Read the active client frame without manufacturing a mutable GameClient
/// reference.  This is safe during `GameClient::update` and fails closed when
/// no client is registered.
pub(crate) fn live_game_client_frame() -> Option<u32> {
    let slot = live_game_client_slot().lock().ok()?;
    (slot.raw.is_some() && slot.owner_thread == Some(std::thread::current().id()))
        .then(|| LIVE_GAME_CLIENT_FRAME.load(Ordering::Acquire))
}

/// Clears the snapshot callback's dynamic exclusive marker during unwind.
struct LiveGameClientBorrowGuard {
    generation: u64,
}

impl Drop for LiveGameClientBorrowGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = live_game_client_slot().lock() {
            if slot.generation == self.generation && slot.raw.is_some() {
                slot.borrowed = false;
            }
        }
    }
}

pub(crate) fn with_live_game_client_mut<R>(
    callback: impl FnOnce(&mut GameClient) -> R,
) -> Option<R> {
    let (raw, _borrow_guard) = {
        let mut slot = live_game_client_slot().lock().ok()?;
        if slot.owner_thread != Some(std::thread::current().id()) || slot.borrowed {
            return None;
        }
        let raw = slot.raw?;
        slot.borrowed = true;
        (
            raw,
            LiveGameClientBorrowGuard {
                generation: slot.generation,
            },
        )
    };
    let client = std::ptr::with_exposed_provenance_mut::<GameClient>(raw);
    // Snapshot adapter only: registration/clear are engine-lifecycle scoped;
    // thread identity and the lexical borrow marker reject cross-thread and
    // re-entrant access.  The guard also clears on panic.
    // SAFETY: `raw` was registered by register_live_game_client from a &mut
    // SAFETY: GameClient that is still engine-owned and un-moved (engine-lifecycle
    // SAFETY: scoped slot). Exclusivity holds because the slot rejects cross-thread
    // SAFETY: access, rejects re-entrance via `borrowed`, and the guard clears the
    // SAFETY: marker on drop/panic before any second borrow can be granted.
    let client = unsafe { client.as_mut()? };
    Some(callback(client))
}

/// C++ `obj->getDrawable()->getCurrentClientBonePositions` via live BasicDrawable.
pub fn query_live_current_client_bone_positions(
    object_id: ObjectID,
    bone_name: &str,
    start: i32,
    max_bones: usize,
) -> Vec<(gamelogic::common::Coord3D, gamelogic::common::Matrix3D)> {
    if max_bones == 0 || bone_name.is_empty() {
        return Vec::new();
    }
    with_live_game_client_mut(|client| {
        let Some(drawable_id) = client.get_drawable_for_object(object_id) else {
            return Vec::new();
        };
        let Some(drawable) = client.find_drawable_by_id(drawable_id) else {
            return Vec::new();
        };
        let Some(basic) = drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
        else {
            return Vec::new();
        };
        let mut positions = vec![crate::drawable::Vector3::zero(); max_bones];
        let mut transforms = vec![crate::drawable::Matrix4::identity(); max_bones];
        let count = basic.get_current_client_bone_positions(
            bone_name,
            start,
            &mut positions,
            &mut transforms,
        );
        (0..count as usize)
            .map(|i| {
                (
                    gamelogic::common::Coord3D::new(positions[i].x, positions[i].y, positions[i].z),
                    transforms[i].to_glam(),
                )
            })
            .collect()
    })
    .unwrap_or_default()
}

fn live_host_object_is_shrouded(object_id: ObjectID) -> bool {
    use gamelogic::common::types::ObjectShroudStatus;
    let player = gamelogic::player::player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .unwrap_or(-1);
    if player < 0 {
        return false;
    }
    let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
    let Ok(shroud) = shroud_manager.lock() else {
        return false;
    };
    match shroud.get_host_object_shroud_status(player as u32, object_id) {
        Some(status) => (status as u8) >= (ObjectShroudStatus::Fogged as u8),
        None => false,
    }
}

/// Live drawable pose for leftover `doFXObj` when leftover `OBJECT_REGISTRY` is empty.
pub fn query_live_drawable_fx_pose(
    object_id: ObjectID,
) -> Option<gamelogic::helpers::HostFxObjectPose> {
    with_live_game_client_mut(|client| {
        let drawable_id = client.get_drawable_for_object(object_id)?;
        let drawable = client.find_drawable_by_id(drawable_id)?;
        let pos = drawable.get_position();
        let xf = drawable.get_transform();
        Some(gamelogic::helpers::HostFxObjectPose {
            id: object_id,
            position: gamelogic::common::Coord3D::new(pos.x, pos.y, pos.z),
            transform: xf.to_glam(),
            player_index: -1,
            bounding_circle_radius: 0.0,
            is_shrouded: live_host_object_is_shrouded(object_id),
        })
    })
    .flatten()
}


/// Capture leftover `BasicDrawable::xfer` visuals for one object-bound drawable.
pub fn capture_live_drawable_xfer_visuals(
    object_id: ObjectID,
) -> Option<crate::drawable::DrawableXferVisualSnapshot> {
    with_live_game_client_mut(|client| {
        let drawable_id = client.get_drawable_for_object(object_id)?;
        let drawable = client.find_drawable_by_id(drawable_id)?;
        drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
            .map(|basic| basic.capture_xfer_visuals())
    })
    .flatten()
}

/// Restore leftover `BasicDrawable::xfer` visuals onto the live GameClient.
pub fn restore_live_drawable_xfer_visuals(
    object_id: ObjectID,
    visuals: &crate::drawable::DrawableXferVisualSnapshot,
) -> bool {
    with_live_game_client_mut(|client| {
        let Some(drawable_id) = client.get_drawable_for_object(object_id) else {
            return false;
        };
        let Some(drawable) = client.find_drawable_by_id_mut(drawable_id) else {
            return false;
        };
        if let Some(basic) = drawable
            .as_any_mut()
            .downcast_mut::<crate::drawable::BasicDrawable>()
        {
            basic.apply_xfer_visuals(visuals);
            true
        } else {
            false
        }
    })
    .unwrap_or(false)
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
    crate::display::shadow_pass::rebuild_shadows();


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

#[cfg(test)]
mod live_slot_tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn live_slot_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn live_snapshot_slot_is_same_thread_reentrant_safe_and_publishes_frame() {
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut client = GameClient::new().expect("client construction");
        client.mark_initialized();
        client.set_frame(41);
        assert_eq!(live_game_client_frame(), Some(41));

        let outer = with_live_game_client_mut(|_| {
            assert!(
                with_live_game_client_mut(|_| ()).is_none(),
                "nested mutable snapshot access must fail closed"
            );
        });
        assert!(outer.is_some());

        let panicked = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_live_game_client_mut(|_| panic!("intentional snapshot callback panic"));
        }));
        assert!(panicked.is_err());
        assert!(
            with_live_game_client_mut(|_| ()).is_some(),
            "the dynamic borrow marker must recover after unwinding"
        );

        clear_live_game_client(&mut client);
        assert_eq!(live_game_client_frame(), None);
    }
}
