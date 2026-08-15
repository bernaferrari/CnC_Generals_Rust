# Unsafe crate allow-list

Workspace lints warn on `unsafe_code`. Do **not** `forbid` workspace-wide until these crates are isolated (hq-ghcr.12).

| Crate / module | Why unsafe exists today | Isolation target |
|---|---|---|
| `game_engine` `common/system/game_memory.rs` | C++ `GameMemory.h`/`GameMemory.cpp` pool: embedded headers, `0xDEADBEEF` free-fill, `*mut u8` user ABI. SlotMap would change pointer identity. Live GameLogic allocate is `Arc`. | keep documented; do not rewrite |
| `wwlib` `ref_ptr.rs` | **deleted** (hq-ghcr.12 first surface; unused COM `*mut T` wrapper). WWSaveLoad already uses `type RefCountPtr<T> = Arc<T>` | done |
| `ww3d-core` `dllist.rs` | **rewritten** to `SlotMap` keys (no raw pointers). C++ `dllist.h` `DLListClass` did not need pointer identity for save/ABI. | done |
| `wwlib` `fast_allocator/` | **rewritten** to `Vec` + index handles. C++ `fastallocator.h` free-list pointers were unused on the game path. | done |
| `game_engine_device` `w3d_c_api/*` | DX8 C ABI (`DrawIndexedPrimitiveUP`, `SetFVF`) | thin adapter over one `GpuContext` |
| window surface (`create_surface_unsafe`) | HWND / window handle | keep, with `SAFETY` + test |

New `unsafe` outside this list needs a `SAFETY:` comment and a reason it cannot live in a listed crate.
