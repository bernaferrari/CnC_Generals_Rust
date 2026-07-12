# Ownership and Authority Policy

> Borrow first, stable IDs second, `Arc` only at real sharing boundaries.

Preserve C++ **behavior**. Do not preserve C++ **pointer ownership**.

## Authoritative simulation

```
OS input → normalized commands → Main GameLogic (30 Hz, temporary host)
  → PresentationFrame (immutable) → GameClient / audio / renderer
```

- **Default**: single Main `GameLogic` authority (`authoritative_world::dual_tick_policy` → `AuthorityOnly`).
- Dual `gamelogic` crate tick is **opt-in** (`GENERALS_ALLOW_DUAL_TICK` only).
- Target end state: `gamelogic` crate `GameWorld` becomes host; Main shrinks to composition root.

## Ownership rules

1. **Owned values + borrowing** — objects/players/teams live in the world; phases take `&mut GameWorld` / `&mut GameLogic`.
2. **Stable IDs** — `ObjectId` / `EntityId` / `PlayerId`; resolve only for the duration of an operation.
3. **`Arc<T>`** — immutable shared resources (templates, meshes, audio buffers).
4. **`Arc<Mutex/RwLock>`** — only true thread boundaries (asset load, audio device, capture).
5. **Channels** — async I/O boundaries; not for splitting the logic frame into Tokio tasks.

## Key types in-tree

| Type | Location | Role |
|------|----------|------|
| `GameLogic` (Main) | `Code/Main/src/game_logic` | Temporary match host (owned HashMaps) |
| `PresentationFrame` | `Code/Main/src/presentation_frame.rs` | Immutable client feed after logic step |
| `GameWorld` + `WorldMutation` | `GameLogic/src/world` | Borrow-first crate target API |
| `AuthorityProbe` | `authoritative_world.rs` | Gate checkpoints |
| `SkirmishMatchConfig` | `skirmish_config.rs` | UI → match rules/slots |

## Migration order

1. PresentationFrame consumers (render/HUD) — stop locking live sim during GPU passes.
2. Unify IDs across Main and crate.
3. Flip crate `HashMap<ObjectID, Arc<RwLock<Object>>>` → owned store + IDs.
4. Retire ObjectFactory dual registry / `engine_object_id` bridge.
5. Rebind GameClient to Main authority + snapshot only.
6. Promote `gamelogic` as sole authority; Main = event loop.

## Gates (honest reading)

| Gate | Proves |
|------|--------|
| `playability_audit` | File mapping only — **not** playability |
| `map_frame_gate` | Logic frames advance (map optional unless assets present) |
| `shell_smoke_gate` | SkirmishMenu→config→apply→map→dual-tick PresentationFrame→HUD selection/minimap→ControlBar.wnd ensure→shell→InGame screen (headless host path; not windowed WND/GPU) |
| `golden_skirmish_gate` | Host vertical slice via AttackObject/update_combat only — no take_damage fallback, no HP caps after spawn |
| `breadth_gate` | Category API smokes |
| `release_candidate_gate` | Soak + presentation smoke + campaign hooks |
| `behavior_gate` | Composite of map+golden+breadth+ai+shell+RC — use this for behavior CI |

### Honest reading (do not overclaim)

- **Proves**: single-host GameLogic authority, skirmish config propagation, production command/combat/save APIs, presentation snapshot fields, retail map load when assets exist.
- **Does not prove**: windowed shell/WND navigation, full GPU match playthrough, complete GameWorld migration, zero GameLogic borrow for FOW/shell bypass, or full W3D mesh-asset retail (unit *identity* for the main mesh pass is presentation-owned; FOW/terrain/asset load remain residual).

Gate honesty labels:

| Gate field | Meaning |
|------------|---------|
| `playable_claim=false` | Must not be read as “retail match is playable end-to-end” |
| shell `playable_claim` | **Always false** — headless APIs ≠ retail W3D/windowed playthrough (fail-closed pending GPU/WND) |
| shell `shell_host_playable_ok` | Limited claim: headless shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd ensure→InGame screen is operational. **Not** full retail play |
| shell `control_bar_layout_ok` | ControlBar.wnd resolve/validate ran (Ready, or honest AssetsUnavailable when WindowZH missing) |
| `synthetic_combat=true` (golden) | Combat/victory on synthetic host world, not Lone Eagle armies |
| `ai_disabled_for_slice=true` (golden) | Opponent AI off so rebuilds do not mask combat failure |
| shell `host_constructed` | True only after `apply_skirmish_config` succeeds (not a constant) |

### Shell residual notes (2026-07)

Closed toward shell_smoke honesty without overclaiming retail W3D:

1. **Dual-tick after StartGame** — map load seeds PresentationFrame; each smoke frame does logic update then `build_and_apply_for_hud` (parity with `start_game_from_ui` / engine dual-tick order).
2. **HUD selection + minimap from presentation** — selection panel health and minimap unit identity come from the snapshot, not live object re-read in the HUD apply path.
3. **ControlBar.wnd ensure** — shell_smoke calls `ensure_control_bar_layout(false)` on the shell→Loading→GameHUD transition (dry-run validate; in-engine `ensure_gameplay_layouts` still attempts window load).
4. **Screen ownership** — UIManager exercises Skirmish → Loading → GameHUD; pregame shell ownership flags are checked for real screen values.
5. **`shell_host_playable_ok` vs `playable_claim`** — success sets the limited host flag; `playable_claim` stays false so gates cannot be misread as “windowed retail match is playable.”

Still residual (not claimed by shell_smoke):

- Windowed shell/WND navigation and GPU match playthrough
- Full ControlBar.wnd parse via WindowManager without GUI init (loaded=true)
- Full W3D retail match playthrough (GPU present, mesh assets, drawables)

### Presentation unit-render residual notes (2026-07)

**Closed (unit identity for main mesh pass):**

1. `PresentationFrame` / `RenderableObject` owns position, orientation, team,
   `team_color`, template/model key, selected, aliveness (`destroyed`),
   `selection_radius`, and `engine_bridged` (RenderBridge skip without live re-read).
2. Production `RenderPipeline::collect_render_items` prefers
   `PresentationFrame::unit_render_inputs()` for the main unit mesh pass when a
   frame is set — no live `GameLogic` object transform/model/selected re-read.
3. Selection overlay (`collect_selected_units_from_presentation`) and HUD/ControlBar
   already consume snapshot identity.
4. Tests: presentation build includes positions/model/team; render collection helper
   builds unit inputs from frame after live world mutation (no logic re-read).

**Still residual (not claimed as full presentation-only renderer):**

| Residual | Why still live / other system |
|----------|-------------------------------|
| FOW visibility alpha / never-explored skip | Pipeline FOW bridge + shell `isInShellGame()` |
| Terrain / heightmap / skybox / roads | Map/environment systems, not unit identity |
| W3D mesh asset resolve / deferred model load | `GraphicsSystem` / `AssetManager` (immutable assets, not sim) |
| RenderBridge drawable path | engine-bridged units drawn outside main mesh pass |
| Full retail W3D GPU match | Fail-closed: not claimed by unit-identity residual close |

