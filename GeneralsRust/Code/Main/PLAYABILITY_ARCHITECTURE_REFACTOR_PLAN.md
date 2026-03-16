# Playability Architecture Refactor Plan

This document defines the implementation strategy to make the Rust game fully playable with C++-faithful behavior while using GPUI for menus/UI overlay on top of world rendering.

Issue tracking is maintained in `bd`:
- `hq-e6u` main playability bug umbrella
- `hq-5qk` architecture epic
- `hq-bl3` diagnostics and observability
- `hq-kg2` menu state authority unification
- `hq-mu7` host integration protocol replacement
- `hq-6wz` shell-map rendering parity

## Current Failure Modes

1. Host composition architecture is fragile:
   - GPUI host runs runtime as a child process and consumes frame files.
   - Frame transport and command/status files create race windows and desync.
2. Menu state ownership is duplicated:
   - GPUI menu state and runtime UI state can diverge.
   - Commands are sent from GPUI but resulting state transitions are partially inferred.
3. Render fidelity and startup parity gaps:
   - Intro shell rendering can appear corrupted relative to C++.
   - Shell camera/bootstrap parity is inconsistent.
4. Observability was insufficient:
   - Logs did not expose frame staleness and fallback transitions with clear counters.

## Target Architecture

The final target is a single coherent composition model:

- One authority for game/menu state (runtime-side state machine).
- GPUI acts as UI layer and interaction surface, not a second gameplay/menu FSM.
- World rendering and GPUI overlay are composed deterministically each frame.
- Transport between world and UI layers is explicit, sequenced, and observable.

## Migration Phases

### Phase 1: Stabilize Existing Host Path

- Improve frame publication and consumption behavior to remove stalls/flicker.
- Add explicit telemetry for frame freshness, invalid frame inputs, and fallback transitions.
- Harden menu exit/navigation command behavior so host and runtime terminate/transition cleanly.

### Phase 2: Unify Menu State Authority

- Runtime publishes canonical screen/menu state.
- GPUI menu becomes a pure projection of runtime state.
- Remove duplicated transition rules from GPUI-side state model.

### Phase 3: Replace File-Based Protocol

- Replace text status/control and file-based coordination with structured protocol.
- Enforce sequence numbers/ack semantics for commands and frame updates.
- Remove non-deterministic file polling races.

### Phase 4: Rendering Parity Pass (C++ Comparison)

- Compare ShellMap bootstrap, camera, object visibility, and texture/material mapping to C++.
- Fix missing/wrong resources and camera/view discrepancies.
- Validate that menu background scene behavior matches C++ shell expectations.

### Phase 5: Playability Closure

- Validate all menu routes are functional and faithful:
  - main, single player, multiplayer, load/replay, options, credits, skirmish/start flow.
- Validate in-game transition from menu to playable state and back.
- Validate startup no-freeze behavior and responsive UI while loading.

## Observability Requirements

Logs must expose:

- Frame publication sequence id and age.
- Last successful frame load timestamp.
- Invalid frame count and cause.
- Fallback activation/deactivation reasons.
- Command dispatch/ack correlation for menu actions.

## Acceptance Criteria

The refactor is complete when:

1. Startup progresses without UI starvation and without indefinite freezes.
2. Menu remains visible, stable, and interactive.
3. World rendering stays visible behind GPUI overlay without black/flicker loops.
4. Command actions produce deterministic runtime transitions.
5. Shell intro/menu visuals and behavior are close to C++ parity.
6. Core playability loop works end-to-end from main menu into gameplay.

