//! Per-session wgpu `request_device` authority (engine `AlreadyInitialised` style).
//!
//! Host owns a [`DeviceAuthority`] (typically on Engine / SharedEngine). First
//! exclusive request on that authority wins; a second is a hard error. Share
//! handles with [`DeviceAuthority::acquire_device`]. No process-wide SLOT.

use crate::GpuError;
use parking_lot::Mutex;

/// Shared wgpu handles produced by the first successful request on an authority.
#[derive(Clone, Debug)]
pub struct SharedGpuDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. The `wgpu::Device`/`wgpu::Queue` fields are `Rc`-backed
// handles on the web backend and therefore `!Send`/`!Sync` by default.
// wasm32-unknown-unknown has no threads, so the handles can never move to or
// be observed from another thread.
unsafe impl Send for SharedGpuDevice {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded.
unsafe impl Sync for SharedGpuDevice {}

#[derive(Debug)]
enum Slot {
    Vacant,
    Pending,
    Occupied(SharedGpuDevice),
}

/// Session device authority — one per Engine / game instance.
#[derive(Debug)]
pub struct DeviceAuthority {
    slot: Mutex<Slot>,
}

impl Default for DeviceAuthority {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceAuthority {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(Slot::Vacant),
        }
    }

    fn begin_exclusive_request(&self) -> Result<(), GpuError> {
        let mut guard = self.slot.lock();
        match *guard {
            Slot::Vacant => {
                *guard = Slot::Pending;
                Ok(())
            }
            Slot::Pending | Slot::Occupied(_) => Err(GpuError::AlreadyInitialised),
        }
    }

    fn complete_exclusive_request(&self, shared: SharedGpuDevice) {
        *self.slot.lock() = Slot::Occupied(shared);
    }

    fn abort_exclusive_request(&self) {
        let mut guard = self.slot.lock();
        if matches!(*guard, Slot::Pending) {
            *guard = Slot::Vacant;
        }
    }

    /// `true` after the first successful [`Self::request_device`] / [`Self::adopt_device`].
    pub fn is_device_acquired(&self) -> bool {
        matches!(*self.slot.lock(), Slot::Occupied(_))
    }

    /// Clone of the session device if one has already been acquired.
    pub fn shared_device(&self) -> Option<SharedGpuDevice> {
        match &*self.slot.lock() {
            Slot::Occupied(shared) => Some(shared.clone()),
            Slot::Vacant | Slot::Pending => None,
        }
    }

    /// Register a device created outside this authority (engine injection / tests).
    pub fn adopt_device(
        &self,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<SharedGpuDevice, GpuError> {
        self.begin_exclusive_request()?;
        let shared = SharedGpuDevice { device, queue };
        self.complete_exclusive_request(shared.clone());
        Ok(shared)
    }

    /// First `Adapter::request_device` on this authority wins.
    pub async fn request_device(
        &self,
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
        self.begin_exclusive_request()?;
        match adapter.request_device(descriptor).await {
            Ok((device, queue)) => {
                self.complete_exclusive_request(SharedGpuDevice {
                    device: device.clone(),
                    queue: queue.clone(),
                });
                Ok((device, queue))
            }
            Err(error) => {
                self.abort_exclusive_request();
                Err(GpuError::RequestDevice(error.to_string()))
            }
        }
    }

    /// Legal sharing path: first caller requests, later callers clone the same handles.
    pub async fn acquire_device(
        &self,
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
        if let Some(shared) = self.shared_device() {
            return Ok((shared.device, shared.queue));
        }
        match self.request_device(adapter, descriptor).await {
            Ok(handles) => Ok(handles),
            Err(GpuError::AlreadyInitialised) => self
                .shared_device()
                .map(|shared| (shared.device, shared.queue))
                .ok_or(GpuError::AlreadyInitialised),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn reset_for_tests(&self) {
        *self.slot.lock() = Slot::Vacant;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_authorities_are_independent() {
        let a = DeviceAuthority::new();
        let b = DeviceAuthority::new();
        assert!(a.begin_exclusive_request().is_ok());
        assert!(b.begin_exclusive_request().is_ok());
        a.abort_exclusive_request();
        b.abort_exclusive_request();
    }

    #[test]
    fn second_request_on_same_authority_returns_already_initialised() {
        let auth = DeviceAuthority::new();
        assert!(auth.begin_exclusive_request().is_ok());
        let error = auth
            .begin_exclusive_request()
            .expect_err("second claim must hard-fail");
        assert!(matches!(error, GpuError::AlreadyInitialised));
        auth.abort_exclusive_request();
    }

    #[test]
    fn failed_request_unclaims_so_retry_is_allowed() {
        let auth = DeviceAuthority::new();
        assert!(auth.begin_exclusive_request().is_ok());
        auth.abort_exclusive_request();
        assert!(auth.begin_exclusive_request().is_ok());
        auth.abort_exclusive_request();
    }
}
