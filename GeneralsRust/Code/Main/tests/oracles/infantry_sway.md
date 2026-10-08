# Infantry wander: signed speed and admitted movement phase

Implementation base: `5ad1b5dac509c7c2574250b9f9d00542ede9b652`, atop upstream
`8360dbea5c9dc2bb8c19b900c951ec8dfbe8e84d`.

## Contract and production change

Original `Locomotor::moveTowardsPositionLegs` advances wander only for TWO_LEGS,
after surviving the blocked/airborne dispatch gates and its downhill-only early
return. With nonzero authored width it adds or subtracts `increment * actualSpeed`,
flips direction only on strict limit crossing, and preserves overshoot. Original
`PhysicsBehavior::getForwardSpeed2D` uses signed component-product magnitude:
`sqrt((vx*dx)^2 + (vy*dy)^2)`, signed by `vx*dx + vy*dy >= 0`. It is not a dot
product or full vector length. Original XY maps to host XZ, with original +Y
corresponding to host -Z.

Main's host fixed tick previously used full nonnegative 3D host speed before the
gates, including every non-Climber appearance with nonzero width. The one-file
production correction moves wander and the dependent angle/delta calculation
after the existing early returns and Thrust exit, before angleCoeff consumption.
It requires `allow_2d_motive`, LegsTwo and nonzero width, and supplies the existing
signed forward-speed helper multiplied by `SECONDS_PER_LOGICFRAME_REAL`. No
continuing-path pose or velocity write is crossed; Climber's PI adjustment stays
in place. The helper, other callers, timers, constructor, RNG and ownership are
unchanged.

## Extracted original arithmetic and rounding boundary

`generate_infantry_sway.py` reads five local original files, requiring their bytes
to match the accepted revision's pins; Git history is not needed. It verifies
complete-file and extracted-span
SHA256 values before compiling the unchanged forward-speed, width-gated wander,
normalizeAngle, Real/PI and frame-conversion definitions. Minimal stubs supply
velocity, explicit phase state and cosf/sinf direction; no original matrix,
constructor, dispatcher, parser or game executable runs. The release-style debug
assert stub preserves the original explicit NaN fallback. Infinity is confined
to speed-only or width-zero controls to avoid nonterminating normalization.

The frozen fixture has 88 rows: 22 live-adapter cases, 25 scalar phase controls,
13 speed controls, 9 normalization controls and 19 sequence steps. The generator
retains exact original spans, generated source, compiler version/hash, command,
flags, binary and output hashes. It uses `-O0 -ffp-contract=off -fno-fast-math`.

Raw-frame rows convert velocity components before the original speed function.
The separately labeled host-adapter lane calculates original signed speed on
host-unit components and then multiplies by original f32(1/30). These orderings
are not bitwise interchangeable: the mixed row has raw speed `3f96d975` and phase
`3e85b65d`, versus adapted speed `3f96d976` and phase `3e85b65e`. Public Rust ticks
compare to the explicitly adapted columns; the fixture preserves both.

## Public runtime evidence

Twenty-seven tests use existing public INI loading, unique authored locomotors,
ThingTemplate registration and GameLogic admission. KindOf::Worker supplies
mobility without Infantry/Vehicle appearance fallback or a WorkerAI module. The
empty world warms through frame 0 before admission; each measured update asserts
one scheduled frame, zero carry, no budget hit and ordinary host authority.
Authored binding fields are inspected and never repaired. Prior wander phase is
installed through existing public object state; this is not constructor or RNG
validation. The blocked gate also injects prior collision state (`is_blocked` and
`cur_max_blocked_speed`) through existing public fields. Public pose, velocity
and move-target APIs provide the airborne/downhill and ordinary motion inputs.
These controlled inputs validate phase handling after admission, not collision
generation or the transition that originally made an object airborne.

Cases cover signed forward/backward/diagonal speed, lateral and vertical controls,
negative heading/width/increment, both phase directions, stationary bias,
strict-limit overshoot and consecutive speed reversal. All eight other
appearances and zero width preserve phase while still moving. Blocked, downhill
and airborne skip->resume tests observe each gate, require unchanged offset and
direction on the skipped frame, then resume from that same phase. Airborne motive
permission is separately authored and exercised.

Phase comparisons are exact. Heading tolerance is 2e-5 for the existing pose and
trigonometric boundary. Nonzero movement/turn/impulse and single X/Z integration
checks establish real host consumers, not full original physics equivalence.
Movement integrates position before the later physics phase adds friction. The
public previous_acceleration exposes that later increment; subtracting it from
final velocity observes marched velocity. Alive, enabled, unstunned and no-bounce
guards exclude later planar mutation or a second acceleration snapshot. The two
supporting tests compare 17 finite nonzero-increment scalar phase samples and
retain the raw-versus-adapted distinction.

## Validation and limits

The first frozen baseline ran 29 tests: 5 passed and 24 failed at the intended
phase assertions. The initial candidate reached those phase/heading expectations
but exposed 11 invalid consumer bounds that measured final post-friction velocity.
That executable and result (18 pass/11 fail) remain recorded. The consumer boundary
was corrected under independent source review while preserving every phase,
heading and fixture expectation. A subsequent 28-pass/1-fail run exposed rounding
in the second-tick velocity setup. Two public add_velocity operations now cancel
the old velocity before adding the exact desired input; its exact assertion stays.

The accepted final test SHA256 is
`a348e321568b5d816c1649cca8a2ec28bc480c03a95466f141c1d8f2123dc285`.
It runs unchanged against both compiled variants: accepted-base production gives
5 pass/24 intended phase failures; the bounded fix gives 29 pass. Distinct native
executables, source hashes/mtimes, rebuilt Main library identities and command logs
are retained. Independent review replayed both results. A restored-old-mtime run
that reused a cached candidate library is explicitly rejected as baseline evidence;
the accepted pair both rebuilt Main after fresh source writes/mtime refresh.

The unchanged adjacent targets pass 96 tests with one existing supplied-file reader
intentionally ignored: signed braking 10, approach braking 11, braking
representation 10, airborne targeting 14, AI pause 9, alliance save/load 13,
experience save/load 6, weapon defaults 15 and deterministic traces 8. LOC/unsafe
ratchets, bounded rustfmt and whitespace checks pass. Existing warnings remain.
The original oracle reproduces byte-identically from a directory containing only
the five pinned originals, generator and fixture, without Git metadata. Corrupted
original bytes and stale file/block pins are rejected before compilation.

The remaining zero-increment fallback, downhill tolerance of 0.05, relative-angle
-PI boundary, broader exceptional/large-angle behavior, initialization length
fallback, appearance overrides and coupled GameWorld movement remain explicit
residuals. `Object::update_movement(dt)` is still a compiled public alternate API
using absolute host speed and LegsTwo|Climber. Its ten current in-tree callers are
cfg(test); the ordinary scheduled GameLogic host path does not call it. That
alternate path is unrepaired, so this is not a fix for all wander paths.
Original gate expectations are
source-derived, not execution of the original dispatcher. No retail visual,
Windows game, asset, save-format, RNG-sequence or ownership parity is claimed.
No lost historical result is reused. bd is unavailable in the task environment.

Reproduce from GeneralsRust with the established environment activated once:

```
python3 Code/Main/tests/oracles/generate_infantry_sway.py --output /tmp/infantry-sway-oracle --verify Code/Main/tests/fixtures/infantry_sway_original.txt
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test infantry_sway -- --nocapture --test-threads=1
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test signed_braking --test approach_braking --test braking_representation --test airborne_targeting --test skirmish_ai_pause --test alliance_save_load --test experience_save_load --test weapon_damage_default --test deterministic_frame_trace_tests --no-fail-fast -- --nocapture --test-threads=1
```

The existing public test graph is the compiled Main package gate. No separate
Main cfg(test), check/release profile, RUSTFLAGS or toolchain change is introduced.
The serial test setting respects existing global stores and makes no parallel
world/store isolation claim.
