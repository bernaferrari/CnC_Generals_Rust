# GeneralsRust Playability Completion Plan (Local/Single-Player)

Status as of `2026-03-02`:

- Matrix: `PORT_FILE_MATRIX.txt` indicates near-complete source/header discovery coverage, but this does not prove runtime parity.
- Tracking artifacts:
  - `PORT_FILE_MATRIX.txt`: `1139` rows (`FOUND=555`, `FOUND_BY_BASENAME=582`, `MISSING=2` in `Precompiled` only).
  - `PORT_MISSING_FILES_BY_SUBSYSTEM.txt`: high-impact subsystems (`Common/GameClient/GameLogic`) currently report `0` source misses.
  - `PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt`: `582` total entries, with `0` high-impact source mismatches when network is deferred.
  - `playability_audit` reports `99.8%` matrix parity and `0` unresolved high-impact blockers (with network deferred).
- Build health:
  - `cargo check --workspace --all-targets --exclude game_network` completes successfully (warnings only).
- Code-level parity debt markers (non-test scan):
  - `Common`: `91` marker hits (`placeholder|stub|not implemented|TODO|FIXME`).
  - `GameClient`: `47` marker hits.
  - `GameLogic`: `29` marker hits.
- Current architecture: network/gameplay architecture is intentionally deferred for multiplayer.

This doc is intentionally concise and directly tied to the four requested execution phases.

## What exists now

### Runtime diagnostics
- `GeneralsRust/Code/Main/src/playability_integration.rs`
  - Parses matrix + missing-list + mismatch-list artifacts.
  - Produces parity summary with per-subsystem totals.
  - Adds phase targets and executable gates.
- Layout mismatches are tracked separately from functional source/header blockers so they do not block strict blocker counts by default.
- `GeneralsRust/Code/Main/src/bin/playability_audit.rs`
  - CLI with `--phase`, `--strict`, and file-path overrides.
- `GeneralsRust/Code/Main/playability_gate.sh`
  - Phase gate runner: runs baseline, gameplay, saveload, ui, release in order.

### Playability status (behavior-centric)

#### Phase 1 — Baseline Lock (currently gated: zero high-impact blockers)
- ✅ Playability audit tooling added.
- ✅ Module/mapping status is machine-readable and repeatable.
- ⚠️  High-impact blockers still include unresolved behavior parity details not directly represented in matrix/mismatch files.

#### Phase 2 — Gameplay Parity Hardening
- ✅ Significant module-wrapper and scheduler work landed in codebase (multiple modules integrated over last updates).
- ✅ Update ordering, sleep semantics, wake frame wiring, and command/action bridges improved.
- ⚠️ Module dispatch completeness and gameplay-edge-case parity still require active parity audit before release.

#### Phase 3 — Save/Load + Terrain Determinism
- ✅ Terrain load/save hooks and object state snapshot/xfer paths have been expanded in prior work.
- ✅ Save/load demo and diagnostics are present.
- ⚠️ Deterministic matrix of 10+ map + 5 object-dense restore cases still missing.

#### Phase 4 — UI / Input Parity
- ✅ Command button validity/ready and several command routes were updated.
- ✅ Purchase science and power flow hooks have parity checks.
- ⚠️ Full control-bar/selection/tooltip/queue edge matrix still open.

#### Phase 5 — Release Candidate
- ✅ Smoke scaffolding and runbook now exists.
- ⚠️ Need one long mixed single-player run target before declaring playable.

## Immediate next actions

1. Replace placeholders in path-layout mismatch handling by migrating these to deterministic module-level tasks:
   - Convert `PORT_FILE_MISMATCHES_BY_SUBSYSTEM` entries into target TODOs in `PLAYABILITY_PLAN.md` issues by subsystem.
2. Add CI-like script target (or manual checklist) for:
   - 30-minute skirmish stability no-panics.
   - Save/load restoration smoke (`10` representative maps).
   - Core control flow matrix for build/repair/upgrade/contain/power/waypoint.
3. Introduce release thresholds in `playability_integration.rs` as behavior-backed constants only after the mismatch backlog is pruned:
   - keep thresholds above current observed values until next checkpoint.

## Known remaining gaps (high-value first)

- Runtime parity still lags matrix parity in several playability-critical paths:
  - weapon runtime has parallel legacy/new stacks (`weapon/mod.rs` and `weapon/weapon_template.rs`) that need consolidation to remove behavioral drift and duplicate scheduling logic;
  - terrain/FX/UI wrappers still carry placeholder behavior in core subsystems (`GameClient/src/core/subsystems.rs`, `terrain/roads.rs`, `terrain/water.rs`, `effects/particles.rs`);
  - INI/audio parsers still include placeholder branches in active data definitions (`Common/src/common/ini/ini_control_bar_scheme.rs`, `ini_damage_fx.rs`, `ini_mapped_image.rs`, `audio/audio_event_rts.rs`);
- Path layout mismatches are now mostly informational include/header addressability debt, not source-coverage blockers.
- Network remains explicitly deferred.
