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
| `shell_smoke_gate` | SkirmishMenu→config→apply→map→frames→PresentationFrame (headless host path; not windowed WND) |
| `golden_skirmish_gate` | Host vertical slice via AttackObject/update_combat only — no take_damage fallback, no HP caps after spawn |
| `breadth_gate` | Category API smokes |
| `release_candidate_gate` | Soak + presentation smoke + campaign hooks |
| `behavior_gate` | Composite of map+golden+breadth+ai+shell+RC — use this for behavior CI |

### Honest reading (do not overclaim)

- **Proves**: single-host GameLogic authority, skirmish config propagation, production command/combat/save APIs, presentation snapshot fields, retail map load when assets exist.
- **Does not prove**: windowed shell/WND navigation, full GPU match playthrough, complete GameWorld migration, presentation-only renderer with zero GameLogic borrow for mesh assets.

Gate honesty labels:

| Gate field | Meaning |
|------------|---------|
| `playable_claim=false` | Must not be read as “retail match is playable end-to-end” |
| `synthetic_combat=true` (golden) | Combat/victory on synthetic host world, not Lone Eagle armies |
| `ai_disabled_for_slice=true` (golden) | Opponent AI off so rebuilds do not mask combat failure |
| shell `host_constructed` | True only after `apply_skirmish_config` succeeds (not a constant) |

