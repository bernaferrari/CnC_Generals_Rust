# Live signed braking force magnitude

This regression covers Main's ordinary public `GameLogic::update_with_dt` route
through the scheduled host locomotor march, then the remaining physics phases.
The production change is only the magnitude comparison in `movement_support.rs`:
`step.abs() > speed_delta.abs()`. The strict comparison and original signed
non-clamping branch are retained. Negative braking below or exactly at the
threshold still accelerates away from the goal, as the original specifies.

## Original authority and native extraction

`generate_signed_braking.py` verifies SHA-256 pins for original INI, GameCommon,
BaseType, Locomotor and PhysicsUpdate sources before extracting and compiling:

- Original `INI::scanReal`, `parseVelocityReal`, `parseAccelerationReal`, and the
  original `GameCommon.h` frame conversion constants/functions
- The unchanged complete `Real speedDelta = ...` force block from
  `Locomotor::moveTowardsPositionOther` (`Locomotor.cpp:2380–2401`), including
  mass multiplication, strict `fabs(force) > fabs(required)` comparison,
  direction multiplication and the applyMotiveForce call
- Original reciprocal-multiply acceleration accumulation from
  `PhysicsBehavior::applyForce`, and original X velocity/position integration
  statements from `PhysicsUpdate.cpp`

The native fixture is an extracted scalar oracle, not a build of the original
Windows game. Stubs supply a finite unit mass, +X direction and motive force
application without the lateral-only projection (the original applyMotiveForce
clears motive before applyForce). Terrain, turning, friction, gravity, collision,
AI and the complete physics body are not executed by that oracle. The Rust public
fixture runs the real host tick and checks the conditions that make these other
phases inert for the observed planar result.

Each live row retains three distinct results:

1. Raw original frame scalars: authored velocity/acceleration use the original
   parsed frame conversion. Authored 90 is `0x40400001` distance/frame
2. Reconstructed original frame scalars: Main's bound host values are divided
   back by 30 (braking twice) and fed to the same extracted original block
3. Explicit host-impulse adapter: speed inputs are host distance/second, and the
   selected acceleration is multiplied by the fixed `1/30` seconds. The original
   force block then chooses the signed impulse at mass one; its updated speed
   and speed times fixed dt are the live Main expected velocity and displacement

Only column 3 is compared to Main. The fixture deliberately retains disagreements
between columns 1 and 2. This does not prove all-bit equivalence of mass
cancellation or seconds/frame round trips. The exact frame-scalar controls are
separate from decimal INI parsing, including strict equality and signed zero.

## Ordinary fixture

The public fixture loads uniquely named OTHER/GROUND locomotors, resolves them,
adds a synthetic template to the public catalog, and calls `create_object`.
It advances an empty world from frame 0 to frame 1 before admission because the
Movement sleepy module first wakes at frame 1. After admission it only uses
`set_orientation`, `add_velocity` and `move_to` to set up the motion, then calls
one public fixed tick. It does not export a test API or repair bound scalars.

The template uses only KindOf::Worker, which the existing `is_mobile` accepts.
A Vehicle draft exposed that current `set_locomotor_physics_options` rewrites
OTHER to Treads during binding. That separate appearance defect is not changed
here. No WorkerAIUpdate, build/repair job or preferred dock is present. No retail
unit-name fallback is used.

Nine live cases start at +X speed 90 toward bound goal 60, except zero-gap starts
at the actual bound goal. The target is (1000,0,0). They cover large, below and
exactly equal negative/positive braking; positive/negative zero rates; and zero
gap. The authored equal-magnitude token is ±899.99981689453125; the test checks
actual bound step and gap bits are equal, rather than assuming authored ±900
lands on that threshold. Only zero-rate cases author MinSpeed=60: the infinite
approach-distance branch then selects the same goal. Other rows use MinSpeed=0.

Every live row checks admission scalars, OTHER appearance, health/mobility, host
rather than coupled shadow authority, one frame/one fixed step, surviving target,
no arrival, no turning/reverse/slide/latch/path/blocked-state interference,
unmodified factor, unit mass, no riders or pending planar acceleration,
zero lateral motion, grounded state, active motive window and no collision.
Use `--test-threads=1` because existing engine stores are process-global;
parallel world/store isolation is not claimed.

## Scope and reproduction

The full force rule is shared by non-Thrust appearance code, but runtime coverage
here is OTHER forward motion only. Reverse decisions, turning/projection,
mass extremes, nonfinite force values, airborne gating, thrust, timer lifecycle,
full deterministic game replay and new save/load behavior are not covered by
this correction. No ownership, RNG or frame-driver code changes.

Run from `GeneralsRust`, with the established environment and test graph:

```
python3 Code/Main/tests/oracles/generate_signed_braking.py --output /tmp/signed-braking-oracle --verify Code/Main/tests/fixtures/signed_braking_original.txt
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test signed_braking -- --test-threads=1
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test approach_braking --test braking_representation -- --test-threads=1
```

The frozen original test/oracle/fixture run against unchanged production has
9 passing tests and one intended failure: large negative braking reaches
150.00002 instead of 60, with one-frame X displacement approximately 5 instead
of 2. The independent reviewer separately replayed that retained executable.
The fixed target passes 10/10 with unchanged regression/oracle/fixture bytes;
large-negative speed is 60 and X displacement is 2. Adjacent approach braking
passes 11/11 and braking representation passes 10/10 on the same test graph.
Bounded rustfmt, LOC/unsafe ratchets and whitespace checks pass. No allowlist
changes, separate Main check/metadata graph, release, Clippy or monolithic Main
cfg(test) build is part of this evidence.
