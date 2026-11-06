// Live GameClient slot, drawable-id counter, and LOD texture reduction.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Snapshot-only address of the engine-owned client.
///
/// `GameClient` contains window/UI state and cannot be owned by a process mutex.
/// The C++ save block nevertheless needs to reach the live instance.  The bridge
/// lives on [`CommonServices::system.live_game_client`] (per-instance), same-thread
/// and dynamically exclusive: normal input/render code must use published scalar
/// state or its lexical `&mut GameClient`, never this slot.
use game_engine::common::common_services::try_common_services;
use game_engine::common::system_stores::LiveGameClientBridge;

const TEXTURE_REDUCTION_MIN: i32 = 0;
const TEXTURE_REDUCTION_MAX: i32 = 4;
const WW3D_TEXTURE_REDUCTION_MIN_DIMENSION: u32 = 32;

fn with_live_bridge_mut<R>(f: impl FnOnce(&mut LiveGameClientBridge) -> R) -> Option<R> {
    let services = try_common_services()?;
    let mut slot = services.system.live_game_client.lock().ok()?;
    Some(f(&mut slot))
}

pub(crate) fn register_live_game_client(client: &mut GameClient) {
    let player_id = client.local_player_id;
    let frame = client.frame;
    let raw = std::ptr::from_mut(client) as usize;
    let registered = with_live_bridge_mut(|slot| {
        if slot.borrowed {
            // Replacing an address while its snapshot callback owns `&mut
            // GameClient` would invalidate the callback's exclusivity.  This is
            // not a valid lifecycle transition, so leave the existing slot live.
            log::error!("refusing to replace a borrowed live GameClient snapshot slot");
            return None;
        }

        slot.generation = slot.generation.wrapping_add(1).max(1);
        slot.token = slot.generation;
        slot.raw = Some(raw);
        slot.owner_thread = Some(std::thread::current().id());
        slot.frame = frame;
        Some(slot.token)
    })
    .flatten();
    let Some(token) = registered else {
        return;
    };
    // Ownership follows this identity, which moves with the value; the address
    // above is only the snapshot access path.
    client.live_slot_token = token;
    crate::terrain::terrain_visual::publish_terrain_visual(std::sync::Arc::clone(
        &client.terrain_visual_gpu,
    ));
    // Free-function player-id publish may re-enter `with_live_game_client_mut`;
    // never call it while the live bridge mutex is held.
    set_local_player_id(player_id);
}

/// Re-anchor the snapshot access path after the owner may have been moved
/// (C++ TheGameClient never moves; a Rust value can). Only the registered
/// owner, outside a snapshot callback, may update the address.
pub(crate) fn refresh_live_game_client_address(client: &mut GameClient) {
    let token = client.live_slot_token;
    if token == 0 {
        return;
    }
    let raw = std::ptr::from_mut(client) as usize;
    let current_thread = std::thread::current().id();
    let _ = with_live_bridge_mut(|slot| {
        if slot.token == token && slot.raw.is_some() && !slot.borrowed
            && slot.owner_thread == Some(current_thread)
        {
            slot.raw = Some(raw);
        }
    });
}

/// Return whether this client owned the published slot. Only that client may
/// tear down process-visible presentation state during its destructor.
/// Identity is the client's registration token, so a client that was moved
/// (including `drop(client)`) still clears exactly its own registration and an
/// older client never clears a newer one.
pub(crate) fn clear_live_game_client(client: &mut GameClient) -> bool {
    let token = std::mem::take(&mut client.live_slot_token);
    if token == 0 {
        return false;
    }
    with_live_bridge_mut(|slot| {
        if slot.token == token && slot.raw.is_some() {
            slot.raw = None;
            slot.token = 0;
            slot.owner_thread = None;
            slot.borrowed = false;
            slot.frame = 0;
            true
        } else {
            false
        }
    })
    .unwrap_or(false)
}

pub(crate) fn owns_live_game_client_slot(client: &GameClient) -> bool {
    let token = client.live_slot_token;
    let current_thread = std::thread::current().id();
    token != 0
        && with_live_bridge_mut(|slot| {
            slot.token == token && slot.raw.is_some() && slot.owner_thread == Some(current_thread)
        })
        .unwrap_or(false)
}

/// Refresh the scalar frame observable for input translators without borrowing
/// the live singleton.  A non-registered test/local client intentionally does
/// not replace the active engine's value.
pub(crate) fn publish_live_game_client_frame(client: &GameClient) {
    publish_client_frame_for_identity(client.live_slot_token, client.frame);
}

fn publish_client_frame_for_identity(token: u64, frame: u32) {
    if token == 0 {
        return;
    }
    let current_thread = std::thread::current().id();
    let _ = with_live_bridge_mut(|slot| {
        if slot.token == token && slot.raw.is_some() && slot.owner_thread == Some(current_thread) {
            slot.frame = frame;
        }
    });
}

/// Read the active client frame without manufacturing a mutable GameClient
/// reference. Safe during `GameClient::update`; fails closed when unregistered.
pub(crate) fn live_game_client_frame() -> Option<u32> {
    with_live_bridge_mut(|slot| {
        (slot.raw.is_some() && slot.owner_thread == Some(std::thread::current().id()))
            .then_some(slot.frame)
    })
    .flatten()
}

/// Clears the snapshot callback's dynamic exclusive marker during unwind.
struct LiveGameClientBorrowGuard {
    generation: u64,
}

impl Drop for LiveGameClientBorrowGuard {
    fn drop(&mut self) {
        let generation = self.generation;
        let _ = with_live_bridge_mut(|slot| {
            if slot.generation == generation && slot.raw.is_some() {
                slot.borrowed = false;
            }
        });
    }
}

pub(crate) fn with_live_game_client_mut<R>(
    callback: impl FnOnce(&mut GameClient) -> R,
) -> Option<R> {
    let (raw, _borrow_guard) = {
        let acquired = with_live_bridge_mut(|slot| {
            if slot.owner_thread != Some(std::thread::current().id()) || slot.borrowed {
                return None;
            }
            let raw = slot.raw?;
            slot.borrowed = true;
            Some((
                raw,
                LiveGameClientBorrowGuard {
                    generation: slot.generation,
                },
            ))
        })?;
        acquired?
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
    with_live_game_client_mut(|client| {
        client.query_current_client_bone_positions(object_id, bone_name, start, max_bones)
    })
    .unwrap_or_default()
}

impl GameClient {
    /// Actual receiving drawable lookup, without publishing or selecting a client.
    pub fn query_current_client_bone_positions(
        &self,
        object_id: ObjectID,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Vec<(gamelogic::common::Coord3D, gamelogic::common::Matrix3D)> {
        if max_bones == 0 || bone_name.is_empty() {
            return Vec::new();
        }
        let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
            return Vec::new();
        };
        let Some(drawable) = self.find_drawable_by_id(drawable_id) else {
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
    }
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

impl GameClient {
    /// Capture C++ `BasicDrawable::xfer` visuals from this client.
    pub fn capture_drawable_xfer_visuals(
        &self,
        object_id: ObjectID,
    ) -> Option<crate::drawable::DrawableXferVisualSnapshot> {
        let drawable_id = self.get_drawable_for_object(object_id)?;
        let drawable = self.find_drawable_by_id(drawable_id)?;
        drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
            .map(|basic| basic.capture_xfer_visuals())
    }

    /// Restore C++ `BasicDrawable::xfer` visuals onto this client.
    pub fn restore_drawable_xfer_visuals(
        &mut self,
        object_id: ObjectID,
        visuals: &crate::drawable::DrawableXferVisualSnapshot,
    ) -> bool {
        let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
            return false;
        };
        let Some(drawable) = self.find_drawable_by_id_mut(drawable_id) else {
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
    }
}

impl GameClient {
    /// Apply queued logic-fire recoil on this client's presentation drawables.
    /// `update` already owns `&mut self`, so routing back through the live
    /// pointer would create an overlapping mutable borrow of this same value.
    fn apply_queued_weapon_recoils(&mut self) {
        let Some(visual_world) = self.visual_world.as_ref() else {
            return;
        };
        let queued = visual_world.take_weapon_recoils();
        for (object_id, amount, aim_angle) in queued {
            let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
                continue;
            };
            let Some(drawable) = self.find_drawable_by_id_mut(drawable_id) else {
                continue;
            };
            if let Some(basic) = drawable
                .as_any_mut()
                .downcast_mut::<crate::drawable::BasicDrawable>()
            {
                basic.apply_recoil_impulse(amount, aim_angle);
            }
        }
    }
}

/// Recoil matrix for the presentation drawable, if it is not identity.
pub fn recoil_glam_for_object(object_id: u32) -> Option<glam::Mat4> {
    with_live_game_client_mut(|client| {
        let drawable_id = client.get_drawable_for_object(object_id)?;
        let drawable = client.find_drawable_by_id(drawable_id)?;
        let basic = drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()?;
        let matrix = basic.recoil_visual();
        if matrix == crate::drawable::Matrix4::identity() {
            None
        } else {
            Some(matrix.to_glam())
        }
    })
    .flatten()
}

/// Apply a texture-reduction target immediately, mirroring C++ `W3DGameClient::adjustLOD`.
///
/// C++ reference:
/// - `W3DGameClient.cpp` `W3DGameClient::adjustLOD` (clamp [0,4], `WW3D::Set_Texture_Reduction`,
///   `TheTerrainRenderObject->setTextureLOD`)
pub fn live_renderer_3d() -> Option<std::sync::Arc<ww3d_renderer_3d::Renderer3dRuntime>> {
    let services = try_common_services()?;
    let guard = services.system.renderer_3d_any.lock().ok()?;
    let any = guard.as_ref()?;
    std::sync::Arc::clone(any)
        .downcast::<ww3d_renderer_3d::Renderer3dRuntime>()
        .ok()
}

/// Publish the session [`Renderer3dRuntime`] for GameClient LOD / texture quality.
pub fn set_live_renderer_3d(r3d: std::sync::Arc<ww3d_renderer_3d::Renderer3dRuntime>) {
    if let Some(services) = try_common_services() {
        if let Ok(mut guard) = services.system.renderer_3d_any.lock() {
            *guard = Some(r3d);
        }
    }
}


pub fn apply_lod_texture_reduction(target_factor: i32) -> Option<i32> {
    apply_lod_texture_reduction_to(target_factor, &mut |clamped| {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.apply_texture_lod_reduction(clamped);
            }
        }
    })
}

/// Synchronous native operation: the caller lends its actual installed terrain.
/// Definition and renderer changes precede terrain invalidation, then shadows.
pub fn apply_lod_texture_reduction_to(
    target_factor: i32,
    terrain: &mut dyn FnMut(i32),
) -> Option<i32> {
    let owned = live_renderer_3d().unwrap_or_else(|| {
        let owned = std::sync::Arc::new(ww3d_renderer_3d::Renderer3dRuntime::new());
        // C++ WW3D texture reduction is process-global; publish the session so
        // later reads observe the same runtime adjustLOD just wrote.
        set_live_renderer_3d(std::sync::Arc::clone(&owned));
        owned
    });
    apply_lod_texture_reduction_to_session(&owned, target_factor, terrain)
}

pub fn apply_lod_texture_reduction_to_session(
    r3d: &ww3d_renderer_3d::Renderer3dRuntime,
    target_factor: i32,
    terrain: &mut dyn FnMut(i32),
) -> Option<i32> {
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

    let renderer_previous = ww3d_renderer_3d::rendering::texture_quality::texture_reduction(r3d);
    if renderer_previous != clamped as u32 {
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(
            r3d,
            clamped as u32,
            WW3D_TEXTURE_REDUCTION_MIN_DIMENSION,
        );
    }

    if previous != clamped || renderer_previous != clamped as u32 {
        terrain(clamped);
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
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn live_slot_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn live_snapshot_slot_is_same_thread_reentrant_safe_and_publishes_frame() {
crate::test_support::ensure_ambient_host();
let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
let mut client = GameClient::new().expect("client construction");
// a04b05dc9: mark_initialized no longer publishes the live slot.
        register_live_game_client(&mut client);
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

    #[test]
    fn moved_client_is_still_identified_and_cleared_by_explicit_drop() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut client = GameClient::new().expect("client");
        register_live_game_client(&mut client);
        client.set_frame(9);
        assert_eq!(live_game_client_frame(), Some(9));
        // Move the value (address changes), then keep driving it: the owner
        // re-anchors the access path and remains the registered owner.
        let mut moved = Box::new(client);
        moved.set_frame(10);
        assert!(owns_live_game_client_slot(&moved));
        assert_eq!(live_game_client_frame(), Some(10));
        assert_eq!(with_live_game_client_mut(|c| c.frame), Some(10));
        // `drop(value)` moves before Drop; ownership is by identity so the slot
        // is cleared rather than left naming a freed address.
        drop(*moved);
        assert_eq!(live_game_client_frame(), None);
        assert!(with_live_game_client_mut(|_| ()).is_none());
    }

    #[test]
    fn stale_older_client_cannot_clear_or_publish_into_a_newer_registration() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut older = Box::new(GameClient::new().expect("older"));
        let mut newer = Box::new(GameClient::new().expect("newer"));
        register_live_game_client(older.as_mut());
        register_live_game_client(newer.as_mut());
        newer.set_frame(77);
        assert!(!owns_live_game_client_slot(&older));
        assert!(owns_live_game_client_slot(&newer));
        older.set_frame(5);
        assert_eq!(live_game_client_frame(), Some(77));
        assert!(!clear_live_game_client(older.as_mut()));
        drop(older);
        assert_eq!(live_game_client_frame(), Some(77));
        assert!(clear_live_game_client(newer.as_mut()));
        assert_eq!(live_game_client_frame(), None);
    }

    #[test]
    fn explicit_drawable_visual_restore_targets_only_its_client() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut live = Box::new(GameClient::new().expect("live client"));
        let mut target = Box::new(GameClient::new().expect("target client"));
        for client in [&mut live, &mut target] {
            let mut drawable = crate::drawable::BasicDrawable::new(DrawableId::INVALID);
            drawable.set_object_id(Some(73));
            client
                .register_drawable_with_template(Box::new(drawable), Some("Test".into()))
                .expect("register object drawable");
        }
        live.mark_initialized();
        let original = live
            .capture_drawable_xfer_visuals(73)
            .expect("live visuals");
        let visuals = crate::drawable::DrawableXferVisualSnapshot {
            explicit_opacity: 0.42,
            hidden: true,
            ..original.clone()
        };
        assert!(target.restore_drawable_xfer_visuals(73, &visuals));
        let restored = target
            .capture_drawable_xfer_visuals(73)
            .expect("target visuals");
        assert_eq!(restored.explicit_opacity, 0.42);
        assert!(restored.hidden);
        let live_after = live
            .capture_drawable_xfer_visuals(73)
            .expect("live visuals remain");
        assert_eq!(live_after.explicit_opacity, original.explicit_opacity);
        assert_eq!(live_after.hidden, original.hidden);
    }

    #[test]
    fn two_clients_own_independent_terrain_visual_gpu_arcs() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut a = GameClient::new().unwrap();
        let mut b = GameClient::new().unwrap();
        assert!(!std::sync::Arc::ptr_eq(
            &a.terrain_visual_gpu,
            &b.terrain_visual_gpu
        ));
        *a.terrain_visual_gpu.lock().unwrap() =
            Some(crate::terrain::terrain_visual::TerrainVisualSystem::new());
        assert!(a.terrain_visual_gpu.lock().unwrap().is_some());
        assert!(b.terrain_visual_gpu.lock().unwrap().is_none());
        register_live_game_client(&mut a);
        {
            let guard =
                crate::terrain::terrain_visual::get_terrain_visual().expect("published a");
            assert!(guard.is_some());
        }
        register_live_game_client(&mut b);
        {
            let mut guard =
                crate::terrain::terrain_visual::get_terrain_visual().expect("published b");
            assert!(guard.is_none(), "b starts empty; a must not be published");
            *guard = Some(crate::terrain::terrain_visual::TerrainVisualSystem::new());
        }
        assert!(
            a.terrain_visual_gpu.lock().unwrap().is_some(),
            "a keeps its own visual after b becomes live"
        );
        assert!(b.terrain_visual_gpu.lock().unwrap().is_some());
        assert!(clear_live_game_client(&mut b));
        crate::terrain::terrain_visual::unpublish_terrain_visual(&b.terrain_visual_gpu);
        // After clear, free-function peels must not observe a foreign client's Arc.
        assert!(
            a.terrain_visual_gpu.lock().unwrap().is_some()
                && b.terrain_visual_gpu.lock().unwrap().is_some()
        );
    }

    #[test]
    fn only_live_client_destroys_terrain_visual_after_drawables() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut old_client = Box::new(GameClient::new().expect("old client"));
        old_client.mark_initialized();
        let mut live_client = Box::new(GameClient::new().expect("live client"));
        register_live_game_client(live_client.as_mut());
        live_client.mark_initialized();

        {
            let mut terrain =
                crate::terrain::terrain_visual::get_terrain_visual().expect("terrain lock");
            *terrain = Some(crate::terrain::terrain_visual::TerrainVisualSystem::new());
        }
        drop(old_client);
        assert!(
            crate::terrain::terrain_visual::get_terrain_visual()
                .expect("terrain lock")
                .is_some(),
            "dropping an older client must not clear the active visual"
        );
        drop(live_client);
        assert!(
            crate::terrain::terrain_visual::get_terrain_visual()
                .expect("terrain lock")
                .is_none(),
            "C++ GameClient destructor deletes its terrain visual"
        );
        let next_client = GameClient::new().expect("next client");
        assert!(
            crate::terrain::terrain_visual::get_terrain_visual()
                .expect("terrain lock")
                .is_none(),
            "constructing a client must not inherit or publish presentation state"
        );
        drop(next_client);
    }

    #[test]
    fn reset_and_active_drop_clear_effects_without_old_client_interference() {
        crate::test_support::ensure_ambient_host();
        let _serial = live_slot_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _tracer_serial = crate::effects::tracer_fx::lock_tracer_fx_tests();

        fn seed_effects(client: &mut GameClient) {
            assert!(crate::effects::ray_effect_system::add_ray_effect(
                901,
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
            ));
            assert!(
                crate::effects::tracer_fx::create_tracer_fx(
                    "GenericTracer",
                    [0.0, 0.0, 0.0],
                    [20.0, 0.0, 0.0],
                    1.0,
                    2.0,
                    1.0,
                    [1.0, 0.0, 0.0],
                    1.0,
                    0,
                )
                .is_some()
            );
            assert!(client.fx_mut().create_display_light_pulse(
                crate::fx_list::DisplayLightPulse {
                    pos: [0.0, 0.0, 0.0],
                    color: [1.0, 0.5, 0.25],
                    inner_radius: 1.0,
                    outer_radius: 50.0,
                    increase_frames: 1,
                    decay_frames: 5,
                }
            ));
        }

        fn assert_effect_count(client: &GameClient, count: usize) {
            assert_eq!(
                crate::effects::ray_effect_system::live_ray_effects().len(),
                count
            );
            assert_eq!(crate::effects::tracer_fx::live_tracer_fx().len(), count);
            assert_eq!(client.fx().scene_dynamic_lights().len(), count);
        }

        crate::effects::clear_live_effects_after_drawables();
        let mut old_client = Box::new(GameClient::new().expect("old client"));
        old_client.mark_initialized();
        let mut live_client = Box::new(GameClient::new().expect("live client"));
        register_live_game_client(live_client.as_mut());
        live_client.mark_initialized();
        seed_effects(&mut live_client);
        assert_effect_count(&live_client, 1);

        let mut candidate = GameClient::new().expect("unpublished staged client");
        candidate.reset().expect("unpublished candidate reset");
        assert_effect_count(&live_client, 1);
        drop(candidate);
        assert_effect_count(&live_client, 1);

        old_client.reset().expect("staged candidate reset");
        assert_effect_count(&live_client, 1);
        drop(old_client);
        assert_effect_count(&live_client, 1);

        live_client.reset().expect("next match reset");
        assert_effect_count(&live_client, 0);
        seed_effects(&mut live_client);
        assert_effect_count(&live_client, 1);
        drop(live_client);

        // After the live owner drops, process ray/tracer globals are cleared by Drop
        // hooks; a fresh client starts with an empty owned light scene.
        assert_eq!(crate::effects::ray_effect_system::live_ray_effects().len(), 0);
        assert_eq!(crate::effects::tracer_fx::live_tracer_fx().len(), 0);
        let next_client = GameClient::new().expect("next client");
        assert_eq!(next_client.fx().scene_dynamic_lights().len(), 0);
        drop(next_client);
    }
}

#[cfg(test)]
#[path = "fx_client_bone_tests.rs"]
mod fx_client_bone_tests;

/// Disjoint frame loan. The live-slot token is its comparison identity.
pub struct ClientFrameWriter<'a> {
    frame: &'a mut u32,
    identity: u64,
}
impl ClientFrameWriter<'_> {
    /// Read the admitted frame from this exact receiving Client loan.
    pub fn frame(&self) -> u32 {
        *self.frame
    }

    pub fn set_frame(&mut self, frame: u32) {
        *self.frame = frame;
        publish_client_frame_for_identity(self.identity, frame);
    }
}
/// Read-only loan of this client's actual admitted Drawable associations.
/// No registry copy or presentation snapshot can substitute for this owner.
pub struct NativeDrawablePresence<'a> {
    drawables: &'a drawable_world::DrawableWorld,
}
impl NativeDrawablePresence<'_> {
    pub fn binding_key(
        &self,
        host_epoch: u64,
        object_id: ObjectID,
    ) -> Option<PresentationDirectDrawableBindingKey> {
        self.drawables
            .presentation_direct_drawable_state(host_epoch, object_id)
            .map(|state| state.binding_key)
    }
}
impl GameClient {
    pub fn split_frame_and_snow(
        &mut self,
    ) -> (ClientFrameWriter<'_>, &mut crate::snow::SnowManager) {
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
        )
    }
    pub fn split_frame_snow_and_drawable_presence(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawablePresence<'_>,
    ) {
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
            NativeDrawablePresence {
                drawables: &self.drawables,
            },
        )
    }
    pub fn split_frame_snow_and_drawable_owner(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
    ) {
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
            drawable_owner,
        )
    }
    pub fn split_frame_snow_drawable_and_campaign(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
        &mut crate::gui::campaign_manager::CampaignManager,
    ) {
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.campaign_manager,
        )
    }
    pub fn split_frame_snow_drawable_campaign_and_hints(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::command_hints::CommandHintState,
    ) {
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.campaign_manager,
            &mut self.command_hints,
        )
    }
    /// Split the same Client's runtime fields for one script-bearing host phase.
    /// The caller owns the transient message vector; callbacks drain into this
    /// exact Client's stream before another authored action is examined.
    pub fn split_frame_snow_drawable_script_ui_and_hints<'a>(
        &'a mut self,
        emitted: &'a mut Vec<crate::message_stream::game_message::GameMessage>,
    ) -> (
        ClientFrameWriter<'a>,
        &'a mut crate::snow::SnowManager,
        NativeDrawableOwner<'a>,
        &'a mut crate::gui::command_hints::CommandHintState,
        crate::gui::game_window::WindowEventEffects<'a>,
        &'a mut crate::message_stream::message_stream::MessageStream,
    ) {
        let client_frame = self.frame;
        refresh_live_game_client_address(self);
        let identity = self.live_slot_token;
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        (
            ClientFrameWriter {
                frame: &mut self.frame,
                identity,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.command_hints,
            crate::gui::game_window::WindowEventEffects {
                cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
                control_bar_observation: None,
                menu_audio: None,
                input_provenance:
                    crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
                standalone_view: None,
                client_frame: Some(client_frame),
                logic_frame: None,
                ui: self.subsystem_manager.in_game_ui.as_mut(),
                payloads: Default::default(),
                pending_window_callbacks: Default::default(),
                diplomacy_world: None,
                menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
                manager: None,
                ime: Some(&mut self.ime),
                window_manager_owner: Some(&self.window_manager_owner),
                selected_window: None,
                lent_windows: Vec::new(),
                emitted_messages: emitted,
                pending_control_bar_clicks: None,
                campaign_manager: Some(&mut self.campaign_manager),
                main_menu: Some(&mut self.main_menu),
                menu_world: None,
                menu_host: Some(&mut self.menu_host),
                script_display: Some(&mut self.script_display),
                window_videos: Some(&mut self.window_videos.challenge),
                pending_main_menu_actions: Vec::new(),
            },
            &mut self.message_stream,
        )
    }

    pub fn command_hint_snapshot(&self) -> Vec<crate::gui::command_hints::CommandHint> {
        self.command_hints.snapshot(self.frame)
    }
    pub fn snow_mut(&mut self) -> &mut crate::snow::SnowManager {
        &mut self.snow
    }
    pub fn snow_render_state(&self) -> crate::snow::SnowRenderState {
        crate::snow::SnowRenderState {
            manager: self.snow.clone(),
            definition: self.snow_definition.clone(),
        }
    }
    /// C++ GameLogic::reset deletes definition overrides without refreshing
    /// SnowManager's cached parameters or noise. Only a parsed Weather block
    /// calls updateIniSettings; immutable definition admission is separate.
    pub fn admit_base_snow_definition(&mut self) {
        self.snow_definition = self.snow_base_definition.clone();
    }
    /// Admit one successfully parsed Weather block, including its runtime refresh.
    pub fn admit_snow_definition(&mut self, definition: &crate::snow::WeatherSetting) {
        self.snow_definition = definition.clone();
        self.snow
            .update_ini_settings_with_definition(definition, &mut |low, high| {
                crate::GameClientRandomValue!(low, high)
            });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod receiving_lod_tests {
    #[test]
    fn receiving_lod_callbacks_keep_peers_separate_and_follow_definition_renderer_updates() {
        crate::test_support::ensure_ambient_host();
        const NAME: &str =
            "receiving_lod_callbacks_keep_peers_separate_and_follow_definition_renderer_updates";
        const MARKER: &str = "GENERALS_RECEIVING_LOD_CHILD";
        if std::env::var(MARKER).as_deref() != Ok(NAME) {
            let module = module_path!().split_once("::").unwrap().1;
            let exact = format!("{module}::{NAME}");
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &exact, "--test-threads=1", "--nocapture"])
                .env(MARKER, NAME)
                .output()
                .unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let definitions = game_engine::common::ini::ini_game_data::ensure_global_data();
        definitions.write().texture_reduction_factor = 0;
        let __r3d = std::sync::Arc::new(ww3d_renderer_3d::Renderer3dRuntime::new());
        // Same session as apply_lod / observe (parity with sibling LOD tests).
        super::set_live_renderer_3d(std::sync::Arc::clone(&__r3d));
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(&__r3d, 0, 32);
        // Publish terrain visual through a live GameClient (register publishes it).
        let _live = crate::test_support::park_live_game_client();
        // A real foreign native holder remains unavailable throughout. The
        // receiving operation must not rediscover or try to borrow that slot.
        let _foreign = crate::terrain::terrain_visual::get_terrain_visual().unwrap();
        let mut a = Vec::new();
        let mut b = Vec::new();
        let observe = |value| {
            assert_eq!(definitions.read().texture_reduction_factor, value);
            assert_eq!(
                super::runtime_global_data::read().texture_reduction_factor,
                value
            );
            assert_eq!(
                ww3d_renderer_3d::rendering::texture_quality::texture_reduction(
                    super::live_renderer_3d().as_deref().unwrap_or(__r3d.as_ref()),
                ),
                value as u32
            );
        };
        assert_eq!(
            super::apply_lod_texture_reduction_to(2, &mut |value| {
                observe(value);
                a.push(value);
            }),
            Some(2)
        );
        assert_eq!(a, vec![2]);
        assert!(b.is_empty());
        assert_eq!(
            super::apply_lod_texture_reduction_to(4, &mut |value| {
                observe(value);
                b.push(value);
            }),
            Some(4)
        );
        assert_eq!(a, vec![2]);
        assert_eq!(b, vec![4]);
        assert_eq!(
            super::apply_lod_texture_reduction_to(4, &mut |value| a.push(value)),
            Some(4)
        );
        assert_eq!(
            a,
            vec![2],
            "unchanged target preserves existing no-invalidation branch"
        );
    }
}
