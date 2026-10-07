# Airborne targeting height: fresh omission and literal signed values

Implementation base: `48985fa09cf5c057cfdd9bb86384739e2b85465e`.
Branch: `dot/reconstruct-airborne-height`.

## Behavior and bounded change

Original fresh `LocomotorTemplate` construction assigns `INT_MAX`. `INI::parseInt`
stores the scanned signed value literally; zero and negatives are not sentinels.
The original AIUpdate flag statement sets exactly when sampled float height is
strictly greater than the signed integer threshold converted to float, otherwise
it clears. A float above converted `INT_MAX` still passes.

Four production changes preserve those semantics: Common defaults to MAX; the
GameLogic bridge and Main binding copy the signed scalar; Main's runtime getter
returns every non-MAX scalar, including zero. MAX still uses the existing named
fallback required by standalone Object::new. Object constructors and missing-field
serde defaults remain MAX. There is no parser grammar, bool, comparison, physics,
frame-driver, authority, ownership, RNG or save-format change.

## Source and frozen original execution

The preparation packet captured `63e4355` metadata and a later movement-support
change. Its original pins are preserved verbatim. All captured source bytes match
the implementation base `48985fa`, including that movement-support capture.
`source_drift.json` records this without changing the old accepted-head claims.

The generator verifies five exact original SHA256 pins and unique extraction
anchors. It executes original Real/Int typedefs, constructor assignment, complete
parseInt/scanInt bodies, inline getter and complete AIUpdate if/set/else/clear.
The native host harness only stubs token supply, template pointer, sampled height
and status storage. It uses four-byte Int/Real, INT_MAX=2147483647, no fast math and
no contraction. Generated source, extracted blocks, compiler version, exact command,
input tokens and output bytes are retained.

Fifteen complete, in-range decimal inputs cover fresh omission, 0/+0/-0, signed
ordinary values, limits and 2^24 rounding. Each threshold uses its float conversion,
adjacent finite floats, height zero and height one with distinct-bit deduplication;
every sample runs both initial flag states. The fixture has 15 scalar rows and
142 predicate rows. It is an extracted phase execution, not the original game,
terrain, complete locomotor constructor, original loader or full AIUpdate body.

## Public native boundary

The new Main integration target uses existing public APIs and unique fresh names.
Four scalar routes each run 30 comparisons: both public Common loaders, direct
GameLogic bridge, named bridge, and Main resolve/apply. Three predicate matrices
run 568 total comparisons: bound Main 284, standalone named Object::new 142, and
literal unnamed Object 142. Six additional zero controls cover no name, missing
name, and a conflicting resolvable name with both initial flags. Constructors,
missing-field serde, explicit zero/negative/normal/limits roundtrip and clone are
covered. MAX divergence controls retain the existing named fallback ambiguity.

Three ordinary public ticks warm an empty world from frame 0 to 1 before template
registration/admission. Worker mobility preserves OTHER without WorkerAI behavior.
The fixture makes a normal move order, advances exactly one frame to 2 and checks
actual movement, surviving motive/target state and ordinary host authority before
and after. The admitted threshold is read, never repaired. Actual final sampled
height bits must match a frozen original predicate row: zero and omitted finish
at height 1 (3f800000); negative -1 finishes at ground 0 (00000000). The zero witness
asserts a meaningful positive height and target flag set; omission clears it and
negative-ground sets it. Physical-airborne thresholds are not claimed equivalent.

## Baseline evidence

The unchanged production baseline runs all 14 tests: 5 pass and 9 fail as intended.
All matrix loops accumulate errors, so these are executed counts, not hypothetical
coverage. Common has 2/30 scalar differences (omission); direct bridge, named bridge,
and Main each have 6/30 (signed-zero tokens). Bound predicate has 24/284 differences,
literal predicate 12/142, standalone predicate 12/142, and six zero/name controls
fail. The real zero tick admits MAX and returns flag false at sampled height 1;
omission and negative-ground controls pass. No test/fixture adjustment was needed
after the baseline. Native baseline executable and its SHA remain retained locally.

## Limits

Fresh omission is tested. The original override loader may inherit an omitted value;
broader override/redefinition semantics are unverified. Explicit MAX, omitted bound
MAX and unbound MAX share one scalar, so a later named-store edit can affect the
fallback. That provenance ambiguity is deliberately preserved, with executable
controls; this is not complete authoritative binding or snapshot provenance parity.

The decimal corpus excludes malformed/trailing and out-of-range scanf grammar.
Predicate tests include below-terrain scalar inputs; those are not claimed stable
live physics states. Main reaches real movement and physics, but the oracle does
not run original terrain/physics or prove bitwise original movement. No lost
historical input count is reused. No paused RNG or guard-recovery material was read.
No Main internal monolithic test build, separate Main cargo-check/Clippy, release
profile, retail assets, Mac work, push or publication is part of this checkpoint.

The repository's bd executable is unavailable in the activated environment.

## Fixed validation and reproduction

The same frozen regression, oracle generator and fixture bytes pass 14/14 after
the four production edits. All 120 scalar and 568 predicate matrix comparisons
match. The live zero witness now admits 0, preserves 0 through the tick, samples
height 1 and sets the flag. All three live witnesses match original output.

Existing Common locomotor arithmetic passes 16/16; Main approach braking passes
11/11, braking representation 10/10 and signed braking 10/10. Those historical
tests were not edited. LOC/unsafe ratchets report zero violations; bounded
rustfmt and whitespace checks pass. Existing compilation warnings remain.
The fixed public test graph builds and links all changed production layers.

From GeneralsRust after sourcing the established environment:

```
python3 Code/Main/tests/oracles/generate_airborne_targeting.py --output /tmp/airborne-oracle --verify Code/Main/tests/fixtures/airborne_targeting_original.txt
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test airborne_targeting -- --test-threads=1
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p game_engine --test locomotor_numeric_parity -- --test-threads=1
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test braking_representation --test approach_braking --test signed_braking -- --test-threads=1
```

Use one test thread because existing engine stores are process-global. This
checkpoint makes no parallel-world/store isolation claim. The preserved binary
runs use --nocapture as well, recording every matrix count and live sample.

Independent review replayed the retained baseline and fixed binaries with the
same 5-pass/9-fail and 14-pass results. It regenerated the 15/142 original fixture
byte-identically, checked all source pins, and detected six isolated semantic
mutants: non-strict comparison, wrong fresh default, zero-to-MAX repair, missing
set, missing clear, and unintended double comparison. Source drift was rejected
before compilation, and absent/duplicate extraction anchors were rejected.
No blocking findings remain within this bounded scope.
