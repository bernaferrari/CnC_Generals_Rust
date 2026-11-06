# Instance binding model

## Goal

Multiple game instances can live in one process. Mutable simulation and
presentation state must not sit in process-wide `static` / `OnceLock` / `LazyLock`
cells. Each instance owns a services bag; free-function bridges resolve the
**currently published** bag for the calling thread.

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
4. **Live GameClient** — typed client borrows go through
   `CommonServices.system.live_game_client` (per Common instance). Headless
   image collections without a live client hang off the published
   `WorldServices.client_image_collection` slot (type-erased), never a process
   `OnceLock`.

## Budget gate

`scripts/globals_baseline.json` is all zeros. `scripts/count_globals.sh
--compare` (via `GameClient/tests/globals_budget.rs`) **fails if any crate
count rises**. Lower the baseline only when a global is deleted; never raise it
to paper over a new static.

## Prefer explicit ownership

Long-term, delete SCOPED_ACTIVE / ACTIVE_STACK call sites by passing
`&WorldServices` / `&CommonServices` / `&mut GameLogic` down the stack (C++
ownership-through-the-engine). Until then, publish for the duration of a call
batch with `with_*` helpers.
