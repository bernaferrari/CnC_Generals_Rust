# Migration Agent Workflow

Use this workflow for one claimed Bead. Observable C++ behavior is the oracle. Rust safety and structure may improve only when ordering, state, timing, serialization, RNG consumption, side effects, and player-visible results remain unchanged.

## Start

1. Run `bd ready --json`, select one exact file-split Bead or provenance packet, then claim it with `bd update <id> --claim --json`.
2. Read the Bead, the original C++ source, the current Rust implementation, and every test/source-honesty consumer named by the Bead. Start editing only when each acceptance condition has a concrete verification command.

Completion: one Bead is claimed and its C++ authority, Rust targets, and gates are known.

## File split

1. Extract behavior-owned modules with names from the domain or original C++ responsibilities. Preserve item order where it affects initialization, dispatch, serialization, or tests. Keep the original public surface; use the narrowest internal visibility that compiles.
2. Run `validate_rust_split.py <repository-relative-original-rs-path>`. Repair every reported oversized fragment, numbered shard, lost test, public API addition, or stale monolith reference.
3. Run every recommended command printed by the validator plus the Bead's focused tests. Regenerate the LOC allowlist so the ceiling shrinks; the unsafe allowlist changes only when the unsafe inventory actually changes.

Completion: the split validator, package check, focused tests, LOC ratchet, unsafe ratchet, rustfmt, and `git diff --check` all pass.

## Provenance packet

1. List compact packet IDs with `generate_port_review_queue.py --list-packets`, then load one bounded packet with `generate_port_review_queue.py --packet <id>`. Inspect every C++ unit and candidate in that packet; a matching filename is discovery evidence, not review.
2. Add only confirmed production destinations to `PORT_PROVENANCE_REVIEWED.json`. Keep path ownership, symbol/range ownership, and behavioral proof as separate claims.
3. Run the packet's commands. `generate_port_tracking.py` must shrink the queue, and both generated-provenance drift checks must pass. Confirm the packet's units appear once in the reviewed manifest and no longer appear in the unreviewed queue.

Completion: every unit in the packet has an honest reviewed mapping or an exact blocker Bead; generated artifacts are current and no network/test/shim/telemetry code receives implementation credit.

## Behavior fix

1. Reproduce the failing Rust test and locate the original C++ branch, defaults, state changes, timing, and side effects.
2. Make the smallest parity change. Keep WGPU substitution backend-neutral and enum cleanup representation-compatible. Add a focused regression test that fails before the fix.
3. Run the focused test, its containing suite, relevant save/load or deterministic trace gates, and the package check.

Completion: the C++-derived regression is green without weakening assertions or introducing unrelated modernization.

## Finish

Record commands and results in the Bead. Create a `discovered-from` Bead with acceptance criteria for any independent failure. Close the claimed Bead only when every acceptance condition passes; otherwise leave exact remaining evidence in its notes. Then follow the repository landing workflow in `AGENTS.md`.
