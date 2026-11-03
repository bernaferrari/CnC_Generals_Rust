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

2. Keep one authoritative production implementation per gameplay domain.
- Isolate platform and backend differences at explicit interfaces.
- Compare that production implementation directly to C++.

3. Preserve C++ observation order while changing ownership.
- When a refactor also fixes a known mismatch, identify the original C++ branch and an executable regression for that change.
- Preserve identities, timing, serialization, and visible side effects.

4. Prefer compatibility adapters over speculative redesign.
- C++ callback tables -> Rust dispatch maps
- C++ globals/singletons (`TheXxx`) -> fields on the per-game-instance owner (no process statics)
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

## Instance Isolation (project goal)
- The target is the C++ game, faithfully, in Rust + wgpu, with multiple independent game instances able to run in parallel in one process.
- Do not add process-wide mutable state: no new `static` Mutex/RwLock/Atomic/OnceLock-with-interior-mutability, `thread_local!` game state, or `lazy_static!` for per-game data. Immutable constant tables are fine.
- When touching a subsystem that still uses a mutable global, move that state onto its owning instance (see `OWNERSHIP_AND_AUTHORITY.md`) instead of extending the global.

## Verification Hygiene
- Verify behavior by running real game logic (fixtures, deterministic traces, runtime probes against retail assets), not by scanning Rust source text.
- Remove self-referential residual/wave/honesty/hunt audit scaffolding while preserving real runtime behavior and executable behavioral tests. Do not introduce more source-string tables or tests that merely check their own constants.
- Do not commit scratch reports, findings JSON, or probe output to the repo root. Put durable notes in the issue tracker.
- Before opening a new issue, check that it blocks playing a skirmish faithfully or breaks instance isolation; prefer closing or updating an existing issue.
- Do not create a new bead for a sub-gap of an existing survivor/focused tracker; add a checklist line (`id — gap — C++ ref`) to that survivor instead.

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
- C++ singletons become state owned by one game instance; C++ `reset()`/`init()` points map to creating or resetting that instance's state, so single-instance behavior is unchanged.
- Add focused regression tests for fixed parity bugs whenever practical.
- Keep temporary startup/runtime probes small and targeted.

## Scope
- Non-network parity first.
- Do not change multiplayer/network logic until non-network gameplay, rendering, UI, FX, terrain, audio, save/load, and startup shell behavior are fully ported and verified.

## Sealed Modules

Sealed modules are **done**. Do not edit them until the whole port is finished,
except for a cross-module API change that requires an explicit unseal.

Registry: `GeneralsRust/sealed_modules.toml`
Gates: `scripts/check_sealed_modules.sh`, `tests/sealed_modules.rs`,
`scripts/check_sealed_diff.sh <base>`.

To touch a sealed path, the commit message must include:
`Unseal: <path> — <reason>`
where `<path>` is the GeneralsRust-relative sealed path (or a prefix of the
touched file). Without that line, `check_sealed_diff.sh` fails.

### Seal checklist (required before adding a `[[module]]` entry)

Work **module by module**. For each module, compare it side by side with its
C++ original under `GeneralsMD/Code/...`, apply 2026 Rust best practices, then
seal it and do not return until the whole port is done.

1. **Behavior matches C++** — read Rust and C++ side by side; note any intentional
   deviations in the module's docs or the `note` field.
2. **Zero process globals** — no mutable process statics / TLS game state; state
   lives on the owning instance (engine / subsystem struct passed explicitly).
   Truly immutable constant tables (no interior mutability) may remain.
3. **No dead or stub code** — no `todo!` / `unimplemented!` / placeholder stubs
   in production sources.
4. **Clippy clean** for that module's package (or the narrowest package that
   compiles it).
5. **Tests check observable behavior**, not source-string audits.
6. **Public API documented** (module- and pub-item docs where callers need them).

After the checklist passes: add the `[[module]]` row to `sealed_modules.toml`,
lower `scripts/globals_baseline.json` if globals fell, commit, and push.

### Instance binding / allowlisted TLS

`scripts/count_globals.sh` (and `GameClient/tests/globals_budget.rs`) keep every
crate at **0** mutable process statics. The only intentional exceptions:

**thread_local! ACTIVE_STACK / SCOPED_ACTIVE** (scoped Arc publish — not game
state). Rust has no other way to bind a per-thread instance bag without a
process-wide static; the payload is `Arc<…Services>`, not the simulation:

| File | Stack | Holds |
|------|-------|-------|
| `Code/GameEngine/Common/src/common/common_services.rs` | `ACTIVE_STACK` | `Arc<CommonServices>` |
| `Code/Main/src/main_services.rs` | `ACTIVE_STACK` | `Arc<MainServices>` |
| `Code/GameEngine/GameLogic/src/system/engine_stores.rs` | `SCOPED_ACTIVE` | `PublishedOwner` → `WorldServices` / `EngineStores` |

**Static allowlist (debug-only instrumentation, not game state):**

| Name | Reason |
|------|--------|
| `CRC_DEBUG_BAG` | `cfg(feature=debug_crc)` instrumentation bag |
| `PERF_GATHER_REGISTRY` | `cfg(feature=perf_timers)` instrumentation bag |

Do **not** add TLS or process statics for gameplay. Put fields on the owning
`CommonServices` / `WorldServices` / `MainServices` / `GameClient` / `AudioManager`
instance. See `docs/instance_binding.md`.


## Standard for Completion
A subsystem is not done because it "works".
It is done when:
- Rust behavior matches C++ behavior for observable results
- asset and timing behavior match expected legacy order
- regression coverage or runtime verification exists for the risky path
- any cleanup has not changed behavior
