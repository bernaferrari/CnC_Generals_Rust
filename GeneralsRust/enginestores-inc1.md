# EngineStores increment 1 — UpgradeCenter + AI store grouped into a GameLogic-owned context (hq-vqyey)

## What landed

**New module**: `Code/GameEngine/GameLogic/src/system/engine_stores.rs` — `EngineStores { upgrade_center, ai }` owning the two highest-churn C++-inherited globals:

- `upgrade_center` — C++ `TheUpgradeCenter` (UpgradeCenter, Upgrade.h)
- `ai` — C++ `TheAI` (AI.cpp:280) including its `AiData` (`TheAI->getAiData()`)

**Resolution model** (mirrors C++ single-pointer semantics):

- `static ACTIVE: RwLock<Option<Arc<EngineStores>>>` — the active world bundle. Every migrated accessor resolves through it.
- `static PROCESS_LIFETIME: LazyLock<Arc<EngineStores>>` — engine-lifetime fallback (C++ has one process-lifetime engine) used while no world is active: engine boot INI loads, headless snippets, tests that never construct a world.
- `EngineStores::new_for_world()` — fresh bundle: fresh `AI` + upgrade-center **snapshot clone** of the engine-lifetime content under a **fresh lock**.
- `new_for_world_installed()` / `install_active()` / `uninstall_active_if_current()` — world lifecycle wiring.

**World lifecycle wiring** (C++ lifecycle order, GameEngine.cpp:468→480→481):

- `Main` host `GameLogic::new()` (game_logic/game_logic/construct.rs) now installs a fresh bundle as active; `impl Drop for GameLogic` (host.rs) uninstalls it only if still current (a stale world dropping after a newer world cannot deactivate it).
- Consequence: every world/test constructing `GameLogic::new()` gets **fresh stores**; per-world mutations and lock poisoning die with the world.

**Accessor migration** (mechanical, C++-name mapping preserved):

- `THE_AI: Lazy<Arc<RwLock<AI>>>` static → `pub fn the_ai() -> Arc<RwLock<AI>>` (ai/mod.rs), resolving via `engine_stores::the_ai()`.
- `THE_UPGRADE_CENTER` static → `get_upgrade_center()` / `with_upgrade_center{,_mut}` all route via `engine_stores::upgrade_center()`.
- ~330 call sites across gamelogic (~115 files), Main (~15 files), examples: `THE_AI.read()/write()` → `the_ai().read()/write()`; imports updated; guard-borrow sites (let-bound `Arc` temporaries) hoisted to `let ai_store = the_ai();` bindings.
- `take_global_ai_for_world_boundary` / `replace_global_ai_for_world_boundary` (runtime_world_transaction.rs world staging) rewired to contents-swap on the **active** bundle's AI — lock identity preserved, alias-safe, rollback semantics unchanged.
- `UpgradeCenter` gained `#[derive(Clone)]` (fields are all `Arc`/HashMap clones) for the world snapshot.

## Why per-world AI + snapshot-cloned UpgradeCenter is behavior-preserving

- **AI fresh per world**: production already swaps AI contents at every map-load boundary (`RuntimeWorldGlobals` take/replace → `AI::new()`); C++ `GameLogic::clearGameData` resets TheAI per game (GameLogic.cpp:436). No production path caches the `THE_AI` Arc (`THE_AI.clone()` had zero call sites).
- **UpgradeCenter snapshot per world**: retail/scripted definitions loaded into the engine-lifetime store (process fallback) are inherited by every new world, so INI-load-then-world order (production boot, test fixtures registering upgrades before `GameLogic::new()`, e.g. object_ai_combat.rs `register_upgrade_completion_sounds`) keeps working; save/load compatibility is kept because restore re-registers leftover templates by name (Upgrade.cpp:458-463 `findNonConstUpgradeByKey`→`newUpgrade`). Fresh lock per world kills the cross-world poison-cascade class.
- Accessor lock/poison semantics unchanged (`expect("UpgradeCenter lock poisoned")` etc.), so panic surfaces stay honest — but a poisoned lock no longer outlives the world.

## Order-dependence class killed

- AI/AiData wealth contamination (resources_wealthy/poor, structure/team seconds, side_info, max_recruit_distance mutations in one test leaking into later tests) dies: worlds no longer share the AI instance.
- UpgradeCenter lock-poison cascades die: each world holds a fresh lock.
- World->world and world->fallback leakage both die (drop-uninstall restores the fallback for no-world code).

## Verification

- `cargo check -p gamelogic` clean; `cargo check -p generals_main` clean.
- Named guards run serially (`--test-threads=1`): results recorded below (see "Guard results").

## Remainder (documented for next increments)

1. `AI_DATA_STORE` (Common crate `ini_ai_data.rs`) stays a process global — it is the AIData.ini parse-side store in `game_engine`; GameLogic-owned EngineStores cannot absorb a Common-crate store without a Common-side indirection (dependency direction). Tests mutating `enable_repulsors` there are currently self-restoring; proper context fields belong to bead step (2).
2. `gamelogic` crate's own `system::game_logic_impl::GameLogic` does not hold an EngineStores field yet (install points are the Main host world); flipping it is mechanical once Main-side guards stay green.
3. UpgradeCenter per-world *content* freshness (scripted leftover accumulation across worlds) is C++-faithful behavior this increment deliberately preserves; bead step (3) removes the remaining process-global surface (Common `ini_upgrade.rs` UPGRADE_CENTER + shroud) last.
4. Env authority flags (`GENERALS_GAMEWORLD_*`) — bead step (2), not touched here.

## Guard results

All serial (`--test-threads=1`), post-change binary:

| Guard | Result |
|---|---|
| `crates_and_salvage` (incl. retaliation AI-store mutators) | 144/144 ok |
| `ai::cpp_parity` | 73/73 ok |
| `command_executor` (Main) | 198/198 ok |
| `commands::command_processor` (gamelogic) | 24/24 ok |
| `economy` (Main) | 129/129 ok |
| `upgrade::center` (gamelogic unit tests) | 12/12 ok |
| `cargo check -p gamelogic` / `-p generals_main` / `--examples` | clean |

Store-exercising ai_player tests (e.g. `arm_structure_timer_applies_wealth_mods_like_cpp`) pass inside the `ai_player` filter.

## Pre-existing failures found while validating (NOT from this change — proven at HEAD)

- 12 `*_cpp_surface` source-scan tests in `ai/ai_player/tests.rs` fail identically at HEAD: `AI_PLAYER_SRC` includes `mod.rs` itself (module split a8aeb60b3), and mod.rs's own `#[cfg(test)] mod tests;` truncates the scanned `prod` window before any impl content. Reproduced by emulating each test's scan logic against `git show HEAD:` content.
- `on_unit_produced_shortcuts_team_delay` fails solo: early `dual_world_registry_unavailable()` return skips the teamDelay zeroing unless another test populated `OBJECT_REGISTRY` first — an OBJECT_REGISTRY order dependence, unrelated to these stores.
- `ai_player::tests::set_var`/`remove_var` helpers recursed into themselves (stack overflow aborting the filter); fixed here to call `std::env::{set_var,remove_var}` so the module can run at all.
- `Common/src/common/system/radar/terrain.rs` was observed mid-edit by a sibling during validation (transient E0599); not touched here.
