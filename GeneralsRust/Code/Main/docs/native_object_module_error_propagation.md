# Native Object module error propagation

Object previously logged a known behavior-module transfer error and continued into
the next module and Object tail. The native GameLogic loader then continued toward
its explicit object registration. C++ `Common/System/Xfer.cpp::xferVersion` throws
for an unsupported version, unwinding the tagged module traversal in
`GameLogic/Object/Object.cpp::xfer`.

`Object::xfer_checked` now returns `ObjectXferError` with the Object ID, optional
module/helper tag, operation and original detail. Count/tag/block operations,
known helper/module failures and unknown-row skip failures propagate through the
Object traversal. Successful byte order and unknown-module length-based skipping
are unchanged. The legacy unit-returning Snapshot adapter stops and diagnoses the
error; it cannot expose a typed error to its caller.

The checked result reaches the native save/load callers through the existing
trigger-context transfer and Common/System bridge. The registered GameLogic
snapshot bridge uses the same `GameLogic::xfer_native_snapshot` implementation as
the local receiving fixture. Native callers receive mode-appropriate errors before
the loader's explicit registration or later records. Lock failures cannot silently
skip the transfer and proceed to registration.

This is a bounded propagation change. Existing ignored primitive/non-module
errors elsewhere in Object remain outside it. Earlier field mutations, callbacks,
construction and global update-proxy side effects are not rolled back.

## Executed evidence

The default-feature public target `native_object_module_error_propagation` has
eight passing cases, each isolated in a child with a 30-second bound. An independent
replay of the same retained executable also passed all eight.

- The actual AIUpdateInterface wrapper accepts version 4 and rejects version 5
  after exactly one byte. Its complete fixture payload is `[4]`: no bound UnitAI
  runtime or nested state machine is implied.
- Actual direct Object save/load/resave preserves every byte on valid input.
- Changing only the AI wrapper version byte reproduces the old continuation bug;
  the repaired legacy adapter stops before the next known module and Object tail.
- The checked API returns the expected Object ID, AI tag, operation and cause,
  with its cursor immediately after the rejected payload.
- A known constructor-helper version failure preserves helper context and stops.
- An unknown module tag still skips its declared one-byte payload and restores
  the following module and tail correctly.
- Actual file-backed System XferLoad and a local receiving GameLogic load two
  valid current records, preserving exact authored module payloads.
- A malformed first record returns `ReadError` at byte 823; the exact remaining
  1,022 bytes are untouched. The receiving GameLogic has no registered Object,
  only the first destination was constructed, and its following module was not
  transferred. The real System XferLoad deferred-registration hook reports zero
  for the failing snapshot and one for the successful control.

The five selected public suites total **24 passed, 0 failed, 4 existing ignores**:
native Object 8; alliance 13 plus 1 ignore; power, AI roster and Strategy continuation
each 1 plus an ignored process-helper entry. Those helpers are invoked explicitly
by their parent scenarios. Nested child result lines are not added to outer totals.

GameLogic, GameClient and Main were actually rebuilt. The remaining stage reused
those libraries while compiling the five tests and Cargo's 19 existing ordinary
binary targets; those applications were not launched. Common/configuration,
default features and the canonical offline/locked serial profile were unchanged.

## Admission and limits

The fixture authors bounded in-memory armor, empty weapons and native template
catalogs. It checks module types/tags/order, cached-body/module serializer identity,
and exact restored bytes after a reversible admission probe. A nonempty unused
native catalog entry prevents the empty last-damage template lookup from triggering
runtime asset discovery. Native template list/map COW identities are pinned as they
actually exist; the fixture does not repair or normalize the catalog.

The native receiving fixture uses the no-neutral-default-team constructor route
and checks actual destination construction/transfer, not an unknown-template skip.
Both registered-bridge creation branches can re-enter global GameLogic services;
registered-bridge liveness remains unverified. The deferred-registration observation
is the actual System XferLoad hook, not the GameState postprocess list or execution
of postprocessing.

Local native registration allocates companion Drawable identities. The two observed
whole-Object resaves each differed only at byte 66, the serialized Drawable ID.
These unnormalized observations are retained. They are not a whole-native restore
failure claim: the fixture transfers GameLogic without the companion subsystem's
complete save/load lifecycle. Direct Object exact resave remains a mandatory green
control; whole native/Drawable identity equivalence is unverified.

Typed propagation does not catch a parser that returns success after consuming
adjacent fields. The separate old-pickup/new-reader witness and producer/dialect
ambiguity remain integration blockers. No global Xfer version/end-block semantics,
native format fork, bounds policy, RNG ownership, historical-save migration,
transactional rollback or broader upstream integration is included.

Earlier failed fixture admissions, the first compile's four non-Display-status
errors, its collector failure, and the later deliberate bin-allowlist stop remain
separate preserved evidence. They are not counted as successful runtime tests.
