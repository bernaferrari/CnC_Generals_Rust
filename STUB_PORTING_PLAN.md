# Stub/Placeholder Porting Plan (Non-Network)

## Objective
Replace remaining runtime-affecting placeholder/fallback behavior in `Common`, `GameLogic`, `GameClient`, and `GameEngineDevice` so single-player behavior reaches C++-faithful parity.

## Current Baseline (2026-03-02)
- Hard runtime stubs (`todo!`/`unimplemented!`/panic-not-implemented): **0**
- Explicit runtime “not implemented/stub implementation/placeholder implementation”: **0**
- Remaining marker debt (`TODO|FIXME|placeholder|stub`): **138**
- `game_engine_device --features video`: **build-clean**
- `generals_main --all-features`: **build-clean**
- `generals_main --all-features --bin generals`: **build-clean**

## Completed In Latest Pass
- Main all-target build drift fixed:
  - Updated legacy bins to current `winit`/`wgpu`/timing/archive/save-load APIs.
  - Reworked stale dev/demo entrypoints that referenced removed/private APIs.
- Retained gameplay lane stability:
  - `generals` executable still compiles with full feature set.

## Remaining Work (Priority Order)
1. Runtime Fidelity Gaps (High Impact)
- `GameEngineDevice/src/w3d/performance_optimizer.rs`
  - Replace mesh-miss skip behavior with recoverable mesh/resource bridge path.
- `GameEngineDevice/src/w3d/w3d_c_api.rs`
  - Replace simplified C-API compatibility branches with parity-accurate behavior.
- `GameClient/src/core/subsystems.rs`
  - Complete movie playback/state wiring in active runtime path.
- `Common/src/common/ini/ini_mapped_image.rs`
  - Complete parse coverage for C++ parity fields.
- `Common/src/common/audio/audio_event_rts.rs`
  - Close advanced routing/side-effect behavior gaps.

2. Validation and Playability Audits
- Repeat campaign/skirmish/save-load/script parity sweeps after each high-impact closure.
- Focus on deterministic frame behavior and save/load round-trip parity.

3. Device-Layer Decision Point
- Either:
  - fully port/fix `game_engine_device --all-features` W3D lane, or
  - explicitly retire/gate superseded W3D path if not part of active gameplay runtime.

## Non-Goals Until Non-Network Completion
- Multiplayer/network parity (`network_*` stubs intentionally deferred).
