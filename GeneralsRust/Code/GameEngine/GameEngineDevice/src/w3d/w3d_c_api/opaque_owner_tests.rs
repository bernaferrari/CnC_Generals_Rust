//! Opaque adapter ownership tests; these do not initialize a GPU or prove a foreign ABI.

use super::constants::GLOBAL_W3D_DEVICE;
use super::device::*;
use super::materials::{effective_bound_texture_id, resolve_detail_texture_id};
use super::types::W3D_VIEWPORT;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn opaque_devices_keep_viewports_and_binding_metadata_isolated_through_destroy() {
    // SAFETY: Both handles are fresh Box allocations from Create. All stack viewport
    // pointers are aligned and live for the synchronous calls. Borrows end before
    // each allocation is destroyed exactly once, including when assertions panic.
    // This test has no concurrent callers and never dereferences a destroyed handle.
    unsafe {
        let first = W3DDevice_Create();
        assert!(!first.is_null());
        let second = W3DDevice_Create();
        if second.is_null() {
            W3DDevice_Destroy(first);
            panic!("second opaque device creation failed");
        }

        let checks = catch_unwind(AssertUnwindSafe(|| {
            let first_viewport = W3D_VIEWPORT {
                x: 7,
                y: 9,
                width: 640,
                height: 320,
                min_z: 0.2,
                max_z: 0.8,
            };
            let second_viewport = W3D_VIEWPORT {
                x: 1,
                y: 3,
                width: 128,
                height: 256,
                min_z: 0.0,
                max_z: 1.0,
            };
            assert_eq!(W3DDevice_SetViewport(first, &first_viewport), 1);
            assert_eq!(W3DDevice_SetViewport(second, &second_viewport), 1);
            let mut actual = second_viewport;
            assert_eq!(W3DDevice_GetViewport(first, &mut actual), 1);
            assert_eq!(
                (actual.x, actual.y, actual.width, actual.height),
                (7, 9, 640, 320)
            );
            assert_eq!((actual.min_z, actual.max_z), (0.2, 0.8));
            for (pointer, ratio) in [(first, 2.0), (second, 0.5)] {
                let owner = &*pointer;
                owner.runtime.block_on(async {
                    let device = owner.device.read().await;
                    assert!(!device.get_status().await.unwrap().initialized);
                    assert_eq!(device.get_scene().await.camera.aspect_ratio, ratio);
                });
            }

            // Stage 0 and stage 1 remain independent binding metadata; no texture is loaded.
            let owner = &*first;
            {
                let mut bindings = owner.bound_textures.lock().unwrap();
                bindings.insert(0, "primary".into());
                bindings.insert(1, "detail".into());
            }
            assert_eq!(
                effective_bound_texture_id(true, true, Some("primary".into())),
                Some("primary".into())
            );
            assert_eq!(resolve_detail_texture_id(owner), Some("detail".into()));
            assert_eq!(resolve_detail_texture_id(&*second), None);
            assert_eq!(*GLOBAL_W3D_DEVICE.lock().unwrap(), Some(second as usize));
        }));
        assert_eq!(W3DDevice_Destroy(first), 1);
        let survivor = catch_unwind(AssertUnwindSafe(|| {
            assert_eq!(*GLOBAL_W3D_DEVICE.lock().unwrap(), Some(second as usize));
            let mut actual = W3D_VIEWPORT {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                min_z: 0.0,
                max_z: 0.0,
            };
            assert_eq!(W3DDevice_GetViewport(second, &mut actual), 1);
            assert_eq!(
                (actual.x, actual.y, actual.width, actual.height),
                (1, 3, 128, 256)
            );
            assert_eq!((actual.min_z, actual.max_z), (0.0, 1.0));
        }));
        assert_eq!(W3DDevice_Destroy(second), 1);
        assert_eq!(*GLOBAL_W3D_DEVICE.lock().unwrap(), None);
        checks.unwrap();
        survivor.unwrap();
    }
}
