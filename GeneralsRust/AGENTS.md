# AGENTS.md

This repository is a strict behavioral port of Command & Conquer: Generals - Zero Hour from
C++ (`GeneralsMD`) to Rust (`GeneralsRust`).

The C++ game is the behavioral specification. Rust may be safer and cleaner internally, but it
must not change observable behavior.

## Core Rule Order
- C++ behavior first
- Rust safety second
- Rust elegance third
- Never trade 1 for 2 or 3

## Parity Rules
1. Treat C++ as the behavioral spec.
- Match defaults, update order, asset lookup order, save/load semantics, timing, and UI/window draw behavior.
- If Rust and C++ disagree, assume Rust is wrong until proven otherwise.

2. Split subsystems into two layers when needed.
- `compat layer`: mirrors C++ decisions exactly, even if awkward.
- `rust layer`: ownership cleanup, typed helpers, error handling, narrower scopes.
- Do not "improve" behavior in the compat layer.

3. Do not rewrite behavior and structure at the same time.
- First make Rust behave like C++.
- Only refactor after parity is validated.
- Reject refactors that change visible side effects.

4. Prefer compatibility adapters over speculative redesign.
- C++ callback tables -> Rust dispatch maps
- C++ globals/singletons -> thin adapters
- C++ asset search order -> exact ordered candidate lists
- C++ draw callbacks -> semantic bridge, not replacement UI

5. Build differential verification into the code.
- For risky subsystems, add parity tests, runtime probes, audit counters, or exact path-resolution logs.
- Use temporary diagnostics when needed, then remove or narrow them after the issue is understood.

6. Be idiomatic in memory safety, not in gameplay semantics.
- Good changes: ownership cleanup, lock scoping, helper extraction, explicit error types, typed enums when layout is preserved.
- Bad changes: simplified state machines, collapsed legacy branches, changed update/timing order, replaced asset rules, speculative abstractions.

7. Keep repo-wide parity invariants explicit.
Every ported file should be explainable in terms of:
- source C++ file
- mirrored state and defaults
- identical side effects
- verification method used

## Rendering and UI Rules
- Exact draw dispatch matters more than prettier code.
- Preserve legacy callback semantics even when the Rust architecture differs.
- If C++ has separate generic draw behavior and callback behavior, Rust must preserve that split.
- Do not replace missing compatibility behavior with new Rust-only presentation logic.

## Asset Resolution Rules
- Centralize resolution logic.
- One shared resolver should know about:
  - extracted asset roots
  - BIG archive roots
  - language-specific paths
  - mod overrides
  - case-insensitive lookup
- Avoid subsystem-specific ad hoc search rules unless C++ truly does the same thing.

## Refactor Rules
- Refactor only behind locked behavior.
- Once parity is proven, reduce duplication and isolate unsafe compatibility edges.
- Preserve all public/stateful behavior while cleaning implementation details.

## Practical Working Rules
- Compare the original C++ source before changing Rust behavior.
- Preserve enum ordering, flag layouts, frame timing, and save/load format behavior.
- Prefer thin adapters around legacy singleton patterns instead of changing subsystem ownership semantics.
- Add focused regression tests for fixed parity bugs whenever practical.
- Keep temporary startup/runtime probes small and targeted.

## Scope
- Non-network parity first.
- Do not change multiplayer/network logic until non-network gameplay, rendering, UI, FX, terrain, audio, save/load, and startup shell behavior are fully ported and verified.

## Standard for Completion
A subsystem is not done because it "works".
It is done when:
- Rust behavior matches C++ behavior for observable results
- asset and timing behavior match expected legacy order
- regression coverage or runtime verification exists for the risky path
- any cleanup has not changed behavior
