# Game module interfaces

These interfaces describe the current non-network Rust port. C++ remains the behavior
reference. Menus and the HUD continue to use the existing renderer and UI; no GPUI migration
is part of this architecture.

## Dependency direction

| Owner | Responsibility | Dependencies and consumers |
| --- | --- | --- |
| `generals_game_domain` | Canonical host object ID, faction value, combat particle kind, AI difficulty | Depends on serde. Main and Presentation use the same types through reexports. |
| `game_pathfinding` | Grid coordinates, A* search, movement costs, search-local layer and occupancy data | Depends on glam. GameLogic reexports the canonical types; Main reaches them through GameLogic. |
| `generals_presentation` | Frozen events, HUD records, objectives, command buttons, weapon visual dispatch records | Depends on GameDomain, serde and glam. Does not depend on simulation, assets, UI widgets or GPU types. |
| Main `presentation_frame` | Builds the completed frame from simulation and adapts it to existing UI/render consumers | Owns `PresentationFrame`, renderable objects and live-host conversions. Its HUD view borrows existing frame fields. |
| Main `assets` | Finds and loads live assets | Each `AssetManager` owns an `ArchiveFileSystem`, which owns the local resolver and BIG mounts. Existing worker synchronization remains at actual I/O boundaries. |
| Main `ai` | Strategic decisions, build/team queues and synchronous host execution adapters | Read decisions use `AiWorldView`; mutable phases still receive the driving `GameLogic`. Native unit AI remains in GameLogic. |

`Team` in GameDomain is the existing host **faction** enum, not the C++/native Team object
or TeamPrototype. Moving its definition does not merge those different domains. Likewise,
the search-local pathfinding layer enum and the broader GameLogic Common layer enum retain
their different discriminants and conversion rules.

## Where behavior changes belong

For search costs, neighbor gating, line seeding or layer transitions, compare `AIPathfind.cpp`
and `AIPathfind.h`, then edit Pathfinding. World terrain, bridges, object occupancy, coordinate
origin and path admission belong in the existing GameLogic/Main adapters. Cell-center height
is supplied by the caller; a leaf search must never discover an active terrain singleton.

For HUD and rendering changes, edit the Main frame builder or its consumers. Presentation
contains only immutable contracts and pure dispatch decisions. Builders retain frame timing,
event order, asset associations and serialization layout. `hud_read_model()` returns borrowed
slices; callers must not introduce a second deep copy of the completed frame.

For asset precedence, edit the owned resolver/file opening path. C++ `FileSystem::openFile`
tries local files before archives. Resident model caching precedes a fresh load. Once a local
file opens, a decode/read failure is not permission to silently substitute archive bytes.
Archive fingerprint queries still describe the archive, independently of a local override.
Relative virtual paths search configured roots in order; the default roots include the
working directory. Explicit absolute paths remain direct caller inputs.
Production mesh loading receives the selected `AssetManager` explicitly. Graphics composition
and authored model-key selection still have ambient asset-provider lookups; resolver ownership
alone does not establish isolation for those consumers.

For strategic AI queries, use `AiWorldView` and its borrowed domain operations. It has no
`Deref`, mutable-world escape, active-instance selection, TLS or snapshot cache. A subsequent
query observes earlier synchronous commands. `world_commands` executes attack state entry
and drains the owning TeamFactory notifications. The update sequence remains base building,
ready teams, queued teams, team building, upgrades/skills, then bridge repair. Do not delay
these commands into a later frame or sort existing iteration order during a structural change.

Mutable AI execution still has broader world access; native configuration and other ambient
stores have separate ownership migrations tracked in beads. This seam is not evidence that
all globals are gone, every subsystem is isolated, or an AI crate can yet be extracted cleanly.

## Verification

Run the contract crates independently:

```sh
cargo test --locked -p game_pathfinding -p generals_game_domain -p generals_presentation --lib
```

Exercise the existing production consumers and state persistence:

```sh
cargo test --locked -p generals_main --lib -- ai:: presentation_frame:: assets:: save_load:: game_logic::pathfinding:: engine_factory:: ui::objectives:: ui::rts_interface:: --test-threads=1
```

The same-ID AI repair test exercises interleaved worlds and reset. Asset tests exercise two
file-system owners, local overrides, BIG fallback, existence checks and streaming readers.
HUD tests cross the real frame-builder/UI seam. Existing save/load suites cover moved type
identity and the unchanged flattened aggregate; extraction does not establish original C++
snapshot fidelity beyond those tests.

Cross-crate relocation checks use `validate_rust_split.py --extracted-source <path>`.
For this extraction, use `--before-ref 2c7007b48`; a later HEAD contains the facade rather
than the original implementation.
Only reachable local dependencies qualify, and mapped source subtrees must preserve their
own test coverage. Unrelated sibling tests cannot mask lost tests. Also run native/release
and wasm compilation, scoped rustfmt, LOC/unsafe ratchets and `git diff --check`. Gate results
and remaining gaps belong in beads, not a second task list in this document.

The game build checks use Main's default game configuration, including GameClient:

```sh
cargo check --locked -p gamelogic --tests
cargo check --locked -p generals_main --tests
cargo check --locked --release -p generals_main --bin generals
cargo check --locked -p generals_main --target wasm32-unknown-unknown --lib --bin generals
```
