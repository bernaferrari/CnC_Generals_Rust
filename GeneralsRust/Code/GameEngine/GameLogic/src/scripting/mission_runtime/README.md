# Mission runtime ownership

This module contains the C++-ordered mission-script runtime formerly embedded in
Main. Main retains platform action handlers and notification queues. Construction
requires an explicit `ScriptEvaluator` and enable queue; it does not initialize
or publish the active ScriptEngine. The supplied evaluator retains the same engine
identity throughout execution.

The four characterization tests moved with the implementation. They retain dense
root/group ordering, CALL_SUBROUTINE exclusion from the ordinary walk, immediate
enable mutations between declarations, an entered group's sibling-chain gate,
one-shot completion, and exact authored name lookup. A separate test exercises
two runtime owners, inert construction, interleaving, and independent reset.

C++ authorities: `ScriptEngine.cpp` update/findScript/findGroup/ENABLE_SCRIPT/
DISABLE_SCRIPT and `Scripts.cpp` linked declaration order. No new interpreter,
action deferral boundary, or per-frame walker is introduced. The live host still
uses the existing ScriptEngine walk; this runtime retains its existing auxiliary
role and public hook behavior.

The enable queue remains shared with reentrant action handlers. The evaluator and
Main initialization adapter still have engine/global dependencies. Full scripting
instance isolation requires migrating those dependencies, not replacing their
mutexes with hidden interior mutability. Snapshot/Xfer remains on the original
ScriptEngine and authored script types; no new serialization format is introduced.

Gate: `cargo test --locked -p gamelogic --lib scripting::mission_runtime -- --test-threads=1`.
