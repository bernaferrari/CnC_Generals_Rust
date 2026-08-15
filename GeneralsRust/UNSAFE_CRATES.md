# Unsafe crate allow-list

Workspace lints warn on `unsafe_code`. Do **not** `forbid` workspace-wide until these crates are isolated (hq-ghcr.12).

| Crate / module | Why unsafe exists today | Isolation target |
|---|---|---|
| `game_engine` `common/system/game_memory.rs` | C++ pool allocator clone (`0xDEADBEEF`, raw headers) | SlotMap / arena unless save ABI needs addresses |
| `wwlib` `ref_ptr.rs` | **deleted** (hq-ghcr.12 first surface; unused COM `*mut T` wrapper). WWSaveLoad already uses `type RefCountPtr<T> = Arc<T>` | remaining: dllist / pool / fast_allocator |
| `wwlib` `dllist.rs` | Intrusive raw-pointer list | `Vec` + `Option<Key>` / `SlotMap` |
| `wwlib` `fast_allocator.rs` | `MaybeUninit::assume_init` + `alloc::alloc` | `Vec` |
| `game_engine_device` `w3d_c_api/*` | DX8 C ABI (`DrawIndexedPrimitiveUP`, `SetFVF`) | thin adapter over one `GpuContext` |
| window surface (`create_surface_unsafe`) | HWND / window handle | keep, with `SAFETY` + test |

New `unsafe` outside this list needs a `SAFETY:` comment and a reason it cannot live in a listed crate.
