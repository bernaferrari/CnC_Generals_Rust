# AGENTS.md

This repo is a full port of Command & Conquer: Generals – Zero Hour from C++ (GeneralsMD) to Rust
(GeneralsRust). The goal is strict behavioral parity and full playability in Rust. Multiplayer/network
logic is explicitly deferred until all non-network systems are ported and verified.

## What We Are Doing
- Port every C++ file to Rust with 1:1 behavior where possible.
- Track parity at the file, subsystem, and feature level using the port matrices in the repo.
- Prefer idiomatic Rust where it does not change observable behavior.
- Use WGPU, Tokio, and glam for rendering, async, and math where helpful, but keep gameplay logic identical.

## Directory Mapping (CPP -> Rust)
- `GeneralsMD/Code/GameEngine/Source/...` -> `GeneralsRust/Code/GameEngine/<crate>/src/...`
- `GeneralsMD/Code/GameEngine/Include/...` -> `GeneralsRust/Code/GameEngine/<crate>/src/...`
- `GeneralsMD/Code/GameEngineDevice/...` -> `GeneralsRust/Code/GameEngine/GameEngineDevice/...`
- `GeneralsMD/Code/GameEngine/GameClient/...` -> `GeneralsRust/Code/GameEngine/GameClient/...`
- `GeneralsMD/Code/GameEngine/GameLogic/...` -> `GeneralsRust/Code/GameEngine/GameLogic/...`
- `GeneralsMD/Code/GameEngine/Common/...` -> `GeneralsRust/Code/GameEngine/Common/...`

## File Parity Sources
Use these repo files to confirm mapping and missing gaps:
- `PORT_FILE_MATRIX.txt` (source-to-destination mapping)
- `PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt` (files that exist but are mismatched)
- `PORT_MISSING_FILES_BY_SUBSYSTEM.txt` (files not yet ported)
- `PORT_STATE.txt` (ongoing status notes)

## How Each File Matches C++ (Process)
1. Identify the original C++ file in `GeneralsMD/...`.
2. Find the mapped Rust file using the port matrix or by matching names/paths.
3. Port logic in order, preserving:
   - state fields and default values
   - update loop behavior
   - save/load (Snapshot/Xfer) behavior
   - INI parsing and default values
4. Confirm parity via:
   - matching constants and enums
   - matching side effects (audio, FX, particle, decals)
   - matching frame/logic timing

## Rust Conventions for Parity
- Use `Snapshotable`/`Xfer` for save/load parity.
- Preserve enum ordering and flag bit layouts.
- Keep frame counters in logic frames (30 FPS standard).
- Use `Arc<RwLock<...>>` for shared mutable state that mirrors C++ ownership patterns.
- Add thin adapters only when C++ used engine singleton globals.

## Scope Rules
- Do not modify GameNetwork until all non-network parity is done.
- Prefer behavior correctness over API polish.
- Avoid deleting user changes or unrelated diffs.
- Keep progress tracking updated (10/20/30 steps).

## Common Crate Relationships
- GameClient: rendering, UI, visual state, FX.
- GameLogic: gameplay simulation, AI, object behaviors.
- Common: shared types, Xfer/Snapshot, INI parsing.
- GameEngineDevice: W3D device/rendering device ports.

## Current Focus
- Finish all non-network parity issues (rendering, UI, FX, save/load, terrain).
- Ensure save/load of Drawable and related systems matches C++.
- Replace placeholders in rendering/audio/asset pipelines.

