//! Receiving client's RadiusDecal resources. Construction is inert; handles
//! remain module-owned and are invalidated when this client lifetime ends.

use crate::radius_decal::ShadowHandle;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct NativeProjectedDecals {
    pub(crate) manager: crate::radius_decal::ProjectedShadowManager,
    leftover_handles: HashMap<u64, ShadowHandle>,
    leftover_next_id: u64,
}

impl NativeProjectedDecals {
    pub(crate) fn leftover_add(
        &mut self,
        texture: &str,
        radius: f32,
        x: f32,
        y: f32,
        z: f32,
        color_argb: u32,
        opacity: f32,
    ) -> Option<u64> {
        let handle = crate::radius_decal::enqueue_delivery_decal_argb_with_owner(
            &mut self.manager,
            texture,
            radius,
            x,
            y,
            z,
            color_argb,
            opacity,
        )?;
        if self.leftover_next_id == 0 {
            self.leftover_next_id = 1;
        }
        let id = self.leftover_next_id;
        self.leftover_next_id = self.leftover_next_id.wrapping_add(1).max(1);
        self.leftover_handles.insert(id, handle);
        Some(id)
    }

    pub(crate) fn leftover_set_position(&self, id: u64, x: f32, y: f32, z: f32) {
        if let Some(handle) = self.leftover_handles.get(&id) {
            handle.set_position(x, y, z);
        }
    }

    pub(crate) fn leftover_set_opacity(&self, id: u64, opacity: f32) {
        if let Some(handle) = self.leftover_handles.get(&id) {
            let scaled = (opacity.clamp(0.0, 1.0) * 255.0).trunc() as i32;
            handle.set_opacity(scaled);
        }
    }

    pub(crate) fn leftover_set_color(&self, id: u64, color_argb: u32) {
        if let Some(handle) = self.leftover_handles.get(&id) {
            handle.set_color(color_argb);
        }
    }

    pub(crate) fn leftover_release(&mut self, id: u64) {
        if let Some(handle) = self.leftover_handles.remove(&id) {
            handle.release();
            self.manager.cleanup();
        }
    }
}

impl Drop for NativeProjectedDecals {
    fn drop(&mut self) {
        self.leftover_handles.clear();
        self.manager.clear();
    }
}

/// Reach the live client's projected decals, or a cfg(test) fallback so unit
/// tests without a registered GameClient still exercise the same owner type.
pub(crate) fn with_projected_decals_mut<R>(
    f: impl FnOnce(&mut NativeProjectedDecals) -> R,
) -> Option<R> {
    // Probe without consuming `f` — `with_live_game_client_mut` would move the
    // FnOnce into a closure even when no live client is registered.
    if super::live_game_client_frame().is_some() {
        return super::with_live_game_client_mut(|client| {
            f(&mut client.drawables.projected_decals)
        });
    }
    #[cfg(test)]
    {
        return Some(TEST_PROJECTED_DECALS.with(|slot| f(&mut slot.borrow_mut())));
    }
    #[cfg(not(test))]
    {
        let _ = f;
        None
    }
}

#[cfg(test)]
std::thread_local! {
    static TEST_PROJECTED_DECALS: std::cell::RefCell<NativeProjectedDecals> =
        std::cell::RefCell::new(NativeProjectedDecals::default());
}

#[cfg(test)]
pub(crate) fn reset_test_projected_decals() {
    TEST_PROJECTED_DECALS.with(|slot| {
        let mut decals = slot.borrow_mut();
        decals.leftover_handles.clear();
        decals.leftover_next_id = 0;
        decals.manager.clear();
    });
}
