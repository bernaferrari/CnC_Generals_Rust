# EngineStores increment 2 — Common AI_DATA_STORE + Common UPGRADE_CENTER + shroud manager → context-owned (hq-vqyey)

## What landed

**Dependency-direction blocker resolved (inc1 remainder #1).** The two Common-crate parse-side stores keep their *types* in `game_engine` (its INI parser writes them during `AIData`/`Upgrade` block parsing, and `game_engine` cannot depend on `gamelogic`); only their *instances* moved into the GameLogic-owned `EngineStores`. Common now hosts an active-slot + engine-lifetime fallback per store (the same resolution model inc1 established for `EngineStores` itself):

- `Common/src/common/ini/ini_ai_data.rs` — `AI_DATA_ACTIVE: RwLock<Option<Arc<RwLock<AIDataStore>>>>` + `AI_DATA_PROCESS_LIFETIME: LazyLock<...>`; accessors `get_ai_data_store() -> Arc<RwLock<AIDataStore>>` (name preserved; the `'static`-guard accessors `get_ai_data_store`/`get_ai_data_store_mut` guard-returning forms are gone), `process_lifetime_ai_data_store()`, `install_ai_data_store()`, `uninstall_ai_data_store_if_current()`. `AIDataStore` gained `Clone`.
- `Common/src/common/ini/ini_upgrade.rs` — `UPGRADE_CENTER_ACTIVE` + `UPGRADE_CENTER_PROCESS_LIFETIME` (LazyLock seeds `UpgradeCenter::new()+init()`, C++ init-before-Upgrade.ini order); `get_upgrade_center() -> Arc<RwLock<UpgradeCenter>>` (never-None — the pre-init `Option` case no longer exists), `process_lifetime_upgrade_center()`, `install_upgrade_center()`, `uninstall_upgrade_center_if_current()`. `initialize_upgrade_center()` kept (forces the LazyLock). Common `UpgradeCenter` gained `Clone`.

**EngineStores extension.** `GameLogic/src/system/engine_stores.rs` `EngineStores` now owns five stores: existing `upgrade_center` + `ai`, plus
- `ini_upgrade_center: Arc<RwLock<IniUpgradeCenter>>` — engine-lifetime bundle shares the Common process store Arc (INI loads outside any world land in the store gameplay reads); world bundles hold a snapshot clone under a fresh lock;
- `ai_data: Arc<RwLock<AIDataStore>>` — same split;
- `shroud: Arc<Mutex<ShroudManager>>` — engine-lifetime bundle fresh; world bundles snapshot-clone the engine content under a fresh lock (`ShroudManager`/`ShroudGrid` gained `Clone`; all inner types already Clone).

World lifecycle wiring is unchanged from inc1: only `Main` host `GameLogic::new()` calls `new_for_world_installed()` (which now also installs the two Common slots), `impl Drop for GameLogic` calls `uninstall_active_if_current` (which now also clears the Common slots if current — ACTIVE lock released before Common-slot writes to keep a single lock order with install).

**Shroud de-globalized.** `SHROUD_MANAGER: OnceLock<...>` deleted; `get_shroud_manager() -> Arc<Mutex<ShroudManager>>` resolves via `engine_stores::shroud_manager()` (active bundle → engine-lifetime fallback). ~60 call sites across gamelogic/GameClient/Main needed no change (`.lock()` derefs identically); `let`-else and multi-line `.lock().map_err(...)?` sites had the Arc hoisted to a binding (let-else drops scrutinee temporaries early; an `Arc` temporary in a plain `let` chain drops at the statement end while the guard lives).

**Call-site migration (mechanical).** ~25 `get_ai_data_store{,_mut}` sites and ~12 Common-`get_upgrade_center{,_mut}` sites converted from guard-style to Arc+read/write-guard style; `get_upgrade_center_mut`/`get_ai_data_store_mut` accessors removed (clean cutover, no shims); re-export list in `Common/src/common/ini/mod.rs` updated. `command_executor/tests/command_audio_build.rs` source-scan assertion (`w.contains("get_ai_data_store")`) still holds — accessor name preserved.

## Store semantics (behavior-preserving rationale)

- AI: fresh per world (inc1).
- UpgradeCenter (gamelogic), Common UpgradeCenter, Common AIData, shroud: world bundles snapshot-clone the engine-lifetime content under a fresh lock. INI-load-then-world order (production boot, test fixtures seeding stores/grids before `GameLogic::new()`) keeps working; per-world mutations and lock poisoning die with the world. Shroud is snapshot rather than fresh because tests (e.g. `game_logic_impl::tests::partition_xfer_writes_cpp_v2_cell_shroud_not_object_positions`) seed the grid before world construction and production world-staging already performs the per-game fresh via `runtime_world_transaction` take/replace (contents swap on the live bundle — unchanged code, now resolving to the active bundle's shroud).

## Order-dependence class killed

Cross-world contamination of AiData thresholds (enable_repulsors, min/distance group thresholds, side build lists, guard rates, insignificant-buildings), Common UpgradeCenter scripted template/mask registrations, and shroud grid/visibility state now die with the owning world instead of leaking into later tests through the process globals.

## Test-site adaptations required by isolation semantics

Tests that mutate an engine store *before* creating a world and *restore* it *after* world creation previously restored the single global; with world snapshots the restore must target the same context that was mutated. Three tests restructured to drop the world before restoring (world usage wrapped in a scope): `command_executor::tests::command_audio_build::group_path_thresholds_read_aidata_store`, `world_tick::mood::mood_scan_ignores_insignificant_buildings`, `ai::cpp_parity_tests::skirmish_new_map_uses_aidata_side_build_list`. (`retaliation_and_physics::apply_aidata_enable_repulsors_honors_parsed_ini_no` writes false→false = the store default, so its ordering is benign.)

## Guard results (serial `--test-threads=1`, lib profile)

| Guard | Result |
|---|---|
| `gamelogic upgrade::center` | 12/12 ok |
| `ai::cpp_parity_tests` | 73/73 ok |
| `economy` | 129/129 ok |
| `crates` (crates_and_salvage + crates) | 216/216 ok |
| `gameworld_shadow` | 304/304 ok (catalog grew by 2 since inc1) |
| `world_tests` catalog | 8/8 ok |
| `command_executor` | 198/198 ok (re-verified after test fix) |
| `combat` | 965/966 — 1 failure, triaged (below) |
| `cargo check -p game_engine / gamelogic / game-client-rust / generals_main` (and lib-test profiles) | clean |

## Failure triage (combat residual)

`host_mods_combat::host_combat_chinook::tests::live_host_chinooks_unstack_landing_dest` fails in the serial `combat` filter. Not store-related: the chinook unstack path does not read any migrated store, and prior bead (hq-e84zk) already recorded environmental combat-filter failures on this machine (retail INI assets absent), with the filter at 964/966 then vs 965/966 now.

The two `command_executor::validate` failures seen mid-wave (group-path distance / formation move) were caused by test-store restore ordering interacting with the new snapshot semantics: three tests mutated a store before creating a world and restored it after, which with world snapshots restored the world's copy while the engine-lifetime store kept the mutation. Fixed by scoping the world so it drops before the restore (command_audio_build, mood, cpp_parity skirmish); both suites re-verified 198/198 and 73/73 on the settled tree. The brief `ww3d-renderer-3d` compile-red that blocked one verification pass was a concurrent decal-lane sibling edit, unrelated to this increment.

All store-exercising suites are green serially; final whole-workspace validation remains with Main's post-wave gate.

## Remainder (documented)

- `gamelogic` crate's own `system::game_logic_impl::GameLogic` still holds no EngineStores field (install points remain the Main host world) — inc1 remainder #2, mechanical once needed.
- Command-executor filter composition is churning from concurrent waves; the two validate failures above should re-triage on a quiet tree at Main's gate.
