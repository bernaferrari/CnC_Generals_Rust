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


<!-- BEGIN BEADS INTEGRATION -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
