# Strategy Center turret aim and fire parity

Verified on 2026-10-07 against code commit
`7b191e9156f80bfdce2dd5e7c7c381f95cabf810`, based on
`78b2524b7543e060e80fae7fce371d5a2226eb77`.

An active Bombardment Strategy Center could acquire a ground enemy but never
fire. Its late mood pass overwrote the generic turret's body-relative yaw with
a different coordinate convention every frame. At body heading zero and a
positive-X target, the turret repeatedly returned to 90 degrees while AIM
needed zero degrees. Other paths also advanced the turret more than once per
frame, and an already in-range tank attack unnecessarily installed a movement
path that rotated the tank's body.

The ordinary turret Idle/Hold operation now selects the Strategy goal without
turning. The next AIM update owns the authored yaw/pitch step. FIRE consumes
that exact goal after live legality, range, plan, pause and readiness checks.
Independent Strategy mood uses its existing residual discharge implementation;
explicit attacks retain their existing generic weapon path. An explicit order
clears mood ownership even when it repeats the same target.

A body attack delegates aiming only when its selected concrete weapon belongs
to a nonzero-turn turret. An already in-range, unobstructed attack on that
turret also avoids requesting a new movement path unless the squish transition
needs one. Existing paths, out-of-range requests, and non-turret/zero-turn/
off-turret-slot fallbacks keep their previous handling.

Late Strategy processing retains or clears ownership at its existing host
boundary. It detects host/turret goal mismatches. Free acquisition rejects
undetected enemy stealth before installing a goal; Passive's existing
last-damage-source selection remains unchanged.

## Original behavior references

Paths are relative to `GeneralsMD/Code/GameEngine/Source`.

- `GameLogic/AI/TurretAI.cpp:855-876`: mood acquisition installs the goal,
  chooses a weapon and sets mood ownership without assigning angles
- `GameLogic/AI/TurretAI.cpp:1055-1175`: AIM advances authored yaw/pitch and
  requires aim and range success before FIRE
- `GameLogic/Object/Update/AIUpdate.cpp:998,1073-1077`: body update precedes
  one update of each turret
- `GameLogic/AI/AIStates.cpp:4998-5020`: a selected turning turret owns aiming;
  a zero-turn fake turret permits body-aim fallthrough
- `GameLogic/AI/AIStates.cpp:5189-5197,5288-5301`: PRE_ATTACK continues,
  other not-ready states fail, accepted fire notifies once, and exit clears firing
- `GameLogic/Object/Update/AIUpdate.cpp:3395-3417`: an object attack installs
  the attack state and goal without an unconditional path request
- `GameLogic/Object/Update/AIUpdate.cpp:4571-4583`: Passive returns its last
  damage source before the general free-target search

## Verification

All execution used the default-feature public Main test graph, offline and
locked, serial compilation and tests, edition 2024, and
`RUST_MIN_STACK=16777216`. No separate check, internal-feature or release graph
was used. Main genuinely rebuilt for the candidate; unchanged source/library
hashes and nanosecond modification times bound the later broad run to it.

| Coverage | Result |
| --- | --- |
| Frozen public `strategy_center_aim` baseline | 8 passed, 7 failed |
| Same 15 tests on candidate | 15 passed |
| Separate live stealth-loss/reacquisition test | 1 passed |
| Separate command/path boundary tests | 2 passed |
| Existing 14-target public graph | 167 passed, 1 existing ignored |
| Total unique executable tests | 185 passed, 1 ignored |

The ignored test is the unchanged `externally_supplied_save_reader` external
compatibility probe. The aggregate test runs have no filtered or duplicate helper-import cases.
Recovery additionally passed 36 fresh-process repetitions: 28 in the owning
verification and 8 in independent verification. Repetitions are separate from
the 18 unique new tests. Earlier partial and intermittent failing evidence was
retained; it is not counted as acceptance evidence.

The public fixture authors templates and owners before admission, keeps both
players alive with a distant weaponless object, activates Bombardment publicly,
waits through its normal door transition, and then admits the target. It does
not repair live plan, owner, weapon, readiness, health or controller fields.
Per-frame observations include real discharge markers, HP, goal identity,
rotation and readiness. Automatic fire occurs at processed frame 256 with
same-frame residual HP loss; explicit fire occurs at 255 and HP loss at 256.
These timing observations distinguish the existing damage paths.

The LOC and unsafe ratchets, edition-2024 rustfmt on all touched Rust files,
and `git diff --check` pass. After runtime verification, a separate formatting
commit wrapped one pre-existing long call. Its sole parsed leaf-token change
is an optional trailing call-argument comma; all other leaves match the tested
source exactly. The final delivery manifest maps tested and final source hashes.
No runtime rebuild was performed for that formatting-only change.

## Limits

No serialized fields or save layout changed. Existing save/load and deterministic
trace tests passed, but a dedicated Strategy turret save/load round trip was not
added. Legacy internal mood/fire tests were adjusted to the new phase contract
and remain unrun in this public graph.

This does not establish full retail-asset, projectile-flight, or complete turret
parity. Automatic residual HP ownership remains immediate. The independent
faction-based battle-plan lookup remains unchanged. Existing moving-path behavior,
unused off-slot turret motion, and broader Passive stealth timing are outside
this bounded verification. No network, RNG ownership, dependency or schema work
was included.
