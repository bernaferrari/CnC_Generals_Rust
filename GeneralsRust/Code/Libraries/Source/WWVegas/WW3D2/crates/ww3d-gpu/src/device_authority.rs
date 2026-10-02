//! Process-wide wgpu `request_device` authority (engine `AlreadyInitialised` style).
//!
//! First exclusive request wins. A second [`request_device`] is a hard error.
//! Share handles with [`acquire_device`]. This wraps acquisition; it does not
//! replace the ww3d-engine singleton.

use crate::GpuError;
use std::sync::OnceLock;

/// Shared wgpu handles produced by the first successful [`request_device`].
#[derive(Clone, Debug)]
pub struct SharedGpuDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. The `wgpu::Device`/`wgpu::Queue` fields are `Rc`-backed
// handles on the web backend and therefore `!Send`/`!Sync` by default.
// wasm32-unknown-unknown has no threads, so the handles can never move to or
// be observed from another thread; the impl only lets the process-wide
// device slot (`parking_lot::Mutex<Slot>`, `OnceLock`) hand out clones.
unsafe impl Send for SharedGpuDevice {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded, so
// `&SharedGpuDevice` is never accessed from two threads and the handles'
// `!Sync` default is unreachable.
unsafe impl Sync for SharedGpuDevice {}

#[derive(Debug)]
enum Slot {
    Vacant,
    Pending,
    Occupied(SharedGpuDevice),
}

// THREAD: the device slot must outlive every thread (host window/GL side and
// game thread both adopt/clone handles) and holds live wgpu `Device`/`Queue`
// handles — thread-local storage would drop them from a TLS destructor after
// wgpu's own TLS has been destroyed.
fn slot() -> &'static parking_lot::Mutex<Slot> {
    static SLOT: OnceLock<parking_lot::Mutex<Slot>> = OnceLock::new();
    SLOT.get_or_init(|| parking_lot::Mutex::new(Slot::Vacant))
}

fn begin_exclusive_request() -> Result<(), GpuError> {
    let mut guard = slot().lock();
    match *guard {
        Slot::Vacant => {
            *guard = Slot::Pending;
            Ok(())
        }
        Slot::Pending | Slot::Occupied(_) => Err(GpuError::AlreadyInitialised),
    }
}

fn complete_exclusive_request(shared: SharedGpuDevice) {
    *slot().lock() = Slot::Occupied(shared);
}

fn abort_exclusive_request() {
    let mut guard = slot().lock();
    if matches!(*guard, Slot::Pending) {
        *guard = Slot::Vacant;
    }
}

/// `true` after the first successful [`request_device`] / [`adopt_device`].
pub fn is_device_acquired() -> bool {
    matches!(*slot().lock(), Slot::Occupied(_))
}

/// Clone of the process-wide device if one has already been acquired.
pub fn shared_device() -> Option<SharedGpuDevice> {
    match &*slot().lock() {
        Slot::Occupied(shared) => Some(shared.clone()),
        Slot::Vacant | Slot::Pending => None,
    }
}

/// Register a device created outside this crate (engine injection / tests).
///
/// # Errors
///
/// [`GpuError::AlreadyInitialised`] if a device was already requested or adopted.
pub fn adopt_device(device: wgpu::Device, queue: wgpu::Queue) -> Result<SharedGpuDevice, GpuError> {
    begin_exclusive_request()?;
    let shared = SharedGpuDevice { device, queue };
    complete_exclusive_request(shared.clone());
    Ok(shared)
}

/// First `Adapter::request_device` wins. A second call is a hard error.
///
/// # Errors
///
/// * [`GpuError::AlreadyInitialised`] if a device was already requested or adopted
/// * [`GpuError::RequestDevice`] if wgpu rejects the request
pub async fn request_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    begin_exclusive_request()?;
    match adapter.request_device(descriptor).await {
        Ok((device, queue)) => {
            complete_exclusive_request(SharedGpuDevice {
                device: device.clone(),
                queue: queue.clone(),
            });
            Ok((device, queue))
        }
        Err(error) => {
            abort_exclusive_request();
            Err(GpuError::RequestDevice(error.to_string()))
        }
    }
}

/// Legal sharing path: first caller requests, later callers clone the same handles.
///
/// # Errors
///
/// Propagates [`request_device`] errors. If a concurrent first request wins the
/// race, returns that winner's handles instead of [`GpuError::AlreadyInitialised`].
pub async fn acquire_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    if let Some(shared) = shared_device() {
        return Ok((shared.device, shared.queue));
    }
    match request_device(adapter, descriptor).await {
        Ok(handles) => Ok(handles),
        Err(GpuError::AlreadyInitialised) => shared_device()
            .map(|shared| (shared.device, shared.queue))
            .ok_or(GpuError::AlreadyInitialised),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
pub(crate) fn reset_device_authority_for_tests() {
    *slot().lock() = Slot::Vacant;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated(test: impl FnOnce()) {
        static TEST_LOCK: OnceLock<parking_lot::Mutex<()>> = OnceLock::new();
        let _serial = TEST_LOCK.get_or_init(|| parking_lot::Mutex::new(())).lock();
        reset_device_authority_for_tests();
        test();
        reset_device_authority_for_tests();
    }

    #[test]
    fn first_device_claim_succeeds() {
        isolated(|| {
            assert!(begin_exclusive_request().is_ok());
            abort_exclusive_request();
        });
    }

    #[test]
    fn second_request_device_returns_already_initialised() {
        isolated(|| {
            assert!(begin_exclusive_request().is_ok());
            let error = begin_exclusive_request().expect_err("second claim must hard-fail");
            assert!(matches!(error, GpuError::AlreadyInitialised));
            abort_exclusive_request();
        });
    }

    #[test]
    fn failed_request_unclaims_so_retry_is_allowed() {
        isolated(|| {
            assert!(begin_exclusive_request().is_ok());
            abort_exclusive_request();
            assert!(begin_exclusive_request().is_ok());
            abort_exclusive_request();
        });
    }

    #[test]
    fn pending_request_blocks_a_second_request_device() {
        isolated(|| {
            assert!(begin_exclusive_request().is_ok());
            assert!(!is_device_acquired());
            assert!(matches!(
                begin_exclusive_request(),
                Err(GpuError::AlreadyInitialised)
            ));
            abort_exclusive_request();
        });
    }
}
