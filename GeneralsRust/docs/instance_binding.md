# Instance binding model

## Goal

Multiple game instances can live in one process. Mutable simulation and
presentation state must not sit in process-wide `static` / `OnceLock` / `LazyLock`
cells. Each instance owns its mutable state. Existing free-function bridges
resolve scoped services while callers migrate to explicit owner borrows;
scoped publication alone does not establish instance isolation.

## The three scoped stacks

| Stack | Crate | Type | Publish API |
|-------|-------|------|-------------|
| `ACTIVE_STACK` | Common | `Arc<CommonServices>` | `with_common_services` / `install_common_services` |
| `ACTIVE_STACK` | Main | `Arc<MainServices>` | `with_main_services` / `install_main_services` |
| `SCOPED_ACTIVE` | GameLogic | `PublishedOwner` (`Core(EngineStores)` or `Services(WorldServices)`) | `with_active_stores` / `with_world_services` / `install_*` |

Each is a `thread_local!` `RefCell<Vec<…>>` nestable stack. Push on enter, pop on
exit (including unwind). Innermost entry wins for `active_*()` / `try_*()`.

These three TLS stacks are the **only** allowlisted process-adjacent bindings
in `scripts/count_globals.sh`. They hold `Arc` handles to instance bags, not
the game state itself. Gameplay fields live **on** those bags
(`WorldServices`, `CommonServices`, `MainServices`, `GameClient`,
`AudioManager`, …).

## Isolation rules

1. **Two instances on one thread** — nest or switch publish scopes. Outer
   instance is restored when the inner scope ends. Do not leave an instance
   published across unrelated work on that thread if another instance may run.
2. **Across threads** — each thread has its own stack. Publishing on thread A
   never makes that bag visible to thread B. Share state only by passing
   `Arc<…Services>` explicitly.
3. **No process fallback bag** — unscoped `active()` / `active_services()` /
   `common_services()` panic. Constructing a world must not publish a process
   singleton.
4. **Live GameClient** — prefer an explicit client borrow. The remaining
   raw live-client adapter does not prove address stability after a move or
   exclusivity against ordinary client borrows. Registration identity and
   callback reentry guards do not repair that safety boundary.

## Budget gate

`scripts/globals_baseline.json` records current ceilings, not a claim that
the codebase has reached zero globals. `scripts/count_globals.sh --compare`
(via `GameClient/tests/globals_budget.rs`) **fails if any crate count rises**.
Lower the baseline only when a global is deleted; never raise it to hide a new static.

## Prefer explicit ownership

Long-term, delete SCOPED_ACTIVE / ACTIVE_STACK call sites by passing
`&WorldServices` / `&CommonServices` / `&mut GameLogic` down the stack (C++
ownership-through-the-engine). Until then, publish for the duration of a call
batch with `with_*` helpers.

## Client presentation migrations

Contained-selection flashes are queued and drained by the receiving
`GameClient`; selecting the same object ID in another client does not drain
that queue. Selection transitions are captured during owned presentation
synchronization.

Control-group membership is observed from that client's `SelectionTranslator`.
Indicator drawing receives its group style, tactical view and display-string
manager explicitly. Move/drop and same-ID membership regressions cover these
loans, not the whole drawing pipeline: standalone visual facts still consult
`OBJECT_REGISTRY`, and font resolution still reaches the legacy live-client
adapter. Legacy squad writes remain for the unmigrated gameplay consumers.
