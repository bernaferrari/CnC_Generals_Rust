# AI System Parity Analysis Report

> **STALENESS NOTICE** — This report was generated on 2026-03-12. Many items
> marked as "missing" or "incomplete" below have since been **fully implemented**.
> See the "Post-Audit Corrections" section at the bottom for a summary of what
> changed. Do **not** use this document as an authoritative gap list without
> cross-referencing the beads tracker (`bd list`).

**Date:** 2026-03-12
**Branch:** parity-fix
**Analyzed by:** Worker Droid (subagent)

## Executive Summary

The Rust AI implementation has substantial coverage of the C++ original but has several gaps in behavioral parity. The core structure exists for all major subsystems, but many state machine transitions, pathfinding algorithms, and AI player behaviors need completion or refinement.

---

## 1. AI Core System (AI.cpp ↔ ai_core.rs / mod.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp`

### Status: **~85% Parity**

### Key Findings:

| Feature | C++ Location | Rust Location | Status |
|---------|--------------|---------------|--------|
| `TheAI` singleton | AI.cpp:274 | mod.rs (`THE_AI`) | ✅ Implemented |
| `TAiData` config struct | AI.cpp:1-175 | mod.rs (`AiData`) | ✅ Implemented |
| Group management | AI.cpp:385-430 | mod.rs (`AI::create_group`) | ✅ Implemented |
| `findClosestEnemy()` | AI.cpp:563-672 | mod.rs | ⚠️ Partial - priority logic simplified |
| `findClosestAlly()` | AI.cpp:714-750 | mod.rs | ✅ Implemented |
| `findClosestRepulsor()` | AI.cpp:756-785 | mod.rs | ✅ Implemented |
| `getAdjustedVisionRangeForObject()` | AI.cpp:787-870 | mod.rs | ⚠️ Partial - debug visualization missing |

### Issues Found:

1. **Attack Priority Calculation (mod.rs:1700-1760)**
   - C++ calculates `modifier = dist/TheAI->getAiData()->m_attackPriorityDistanceModifier`
   - Rust implementation exists but **garrisoned building priority check is missing**
   - C++ lines 634-654 contain container iteration for higher-priority contained units
   - **Fix Required:** Add `ContainModuleInterface` iteration to check contained unit priorities

2. **CRC/Xfer for AiData (mod.rs:700-780)**
   - `xfer()` only handles version, missing full field serialization
   - C++ xfers all 22+ fields individually
   - **Fix Required:** Complete `Snapshot` implementation for save/load parity

---

## 2. AI State Machine (AIStates.cpp ↔ ai_states.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp`

### Status: **~70% Parity**

### Key Findings:

| State | C++ Lines | Rust Implementation | Status |
|-------|-----------|---------------------|--------|
| `AttackStateMachine` | 145-230 | ai_states.rs | ⚠️ Partial |
| `AIIdleState` | 580-650 | ai_states.rs | ✅ Implemented |
| `AIMoveToState` | 655-750 | ai_states.rs | ⚠️ Missing path validation |
| `AIRappelState` | 320-400 | Not found | ❌ **Missing** |
| `AIAttackAimAtTargetState` | 450-550 | states.rs | ⚠️ Partial |
| `AIAttackFireWeaponState` | 555-620 | states.rs | ⚠️ Partial |
| `AIAttackApproachTargetState` | 625-720 | states.rs | ⚠️ Partial |
| `AIAttackPursueTargetState` | 725-800 | states.rs | ⚠️ Partial |

### Critical Issues:

1. **State Transition Conditions (ai_states.rs:80-150)**
   - C++ uses `StateConditionInfo` arrays for complex conditionals
   - Rust uses simple functions like `out_of_weapon_range_object()`
   - **Missing conditions:**
     - `cannotPossiblyAttackObject()` (C++ line ~200)
     - `inWeaponRangeObject()` 
     - `wantToSquishTarget()` - exists but incomplete squish logic

2. **Attack State Machine Construction (ai_states.rs:1-100)**
   - C++ constructs different state graphs for:
     - Normal attacks vs force attacks
     - Object targets vs position targets
     - Mobile vs immobile units
     - Portable structures
   - Rust implementation lacks these conditional state definitions

3. **Container Enemy Detection (C++ AIStates.cpp:250-280)**
   ```cpp
   static Object* findEnemyInContainer(Object* killer, Object* bldg)
   ```
   - **Not implemented in Rust**
   - Required for units attacking garrisoned buildings

4. **Rappel State (AIRappelState) - COMPLETELY MISSING**
   - C++ lines 320-425
   - Required for infantry rappelling from helicopters
   - Handles `MODELCONDITION_RAPPELLING`, physics reset, destination calculation

---

## 3. AI Pathfinding (AIPathfind.cpp ↔ pathfind.rs, pathfind_astar.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp`

### Status: **~75% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| `PathNode` class | 75-165 | pathfind.rs | ✅ Implemented |
| `Path` class | 170-400 | pathfind.rs | ⚠️ Partial |
| A* algorithm | 2500-3000 | pathfind_astar.rs | ✅ Implemented |
| `Pathfinder` class | 800-1200 | mod.rs | ⚠️ Simplified |
| Zone updates | 4500-5000 | pathfinding_system.rs | ⚠️ Partial |
| Bridge pathfinding | 3500-4000 | Not fully implemented | ❌ **Incomplete** |

### Critical Issues:

1. **Path Optimization (C++ AIPathfind.cpp:380-470)**
   - `Path::optimize()` performs line-of-sight checking
   - C++ uses `isLinePassable()` with layer transitions
   - Rust `path_optimization.rs` exists but **layer handling is simplified**

2. **Closest Point on Path (C++ AIPathfind.cpp:500-600)**
   - `Path::closestPointOnPath()` - complex algorithm for AI movement
   - Handles `m_cpopRecentStart`, `m_cpopCountdown`, `m_cpopValid`
   - **Rust implementation incomplete**

3. **Bridge Pathfinding (C++ AIPathfind.cpp:3500-4000)**
   - `LAYER_GROUND` vs `LAYER_BRIDGE` transitions
   - `CELL_BRIDGE_IMPASSABLE` handling
   - **Rust needs explicit bridge layer support**

---

## 4. AI Player System (AIPlayer.cpp ↔ ai_player.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPlayer.cpp`

### Status: **~80% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| Base construction | 200-400 | ai_player.rs | ⚠️ Partial |
| Team building | 600-900 | ai_player.rs | ✅ Implemented |
| Supply truck queueing | 1200-1400 | ai_player.rs | ⚠️ Partial |
| Repair system | 1600-1800 | ai_player.rs | ⚠️ Partial |
| `onStructureProduced()` | 100-200 | ai_player.rs | ✅ Implemented |
| `queueSupplyTruck()` | 1200-1380 | ai_player.rs | ⚠️ Simplified |

### Critical Issues:

1. **Dozer/Worker Management (C++ AIPlayer.cpp:800-1000)**
   - C++ has complex `findDozer()` and `queueDozer()` logic
   - Handles multiple dozer types (USA, China, GLA worker)
   - **Rust implementation simplified**

2. **Supply Center Checking (C++ AIPlayer.cpp:230-300)**
   ```cpp
   void AIPlayer::checkForSupplyCenter(BuildListInfo *info, Object *bldg)
   ```
   - Determines harvester count based on difficulty
   - **Rust missing difficulty-based gatherer adjustment**

3. **Rebuild Hole Behavior (C++ AIPlayer.cpp:150-200)**
   - GLA structure rebuilding from holes
   - `RebuildHoleBehaviorInterface` integration
   - **Rust implementation missing**

4. **Structure Repair Prioritization (C++ AIPlayer.cpp:1600-1800)**
   - `MAX_STRUCTURES_TO_REPAIR = 2` 
   - Priority queue for damaged structures
   - **Rust has placeholder implementation**

---

## 5. AI Skirmish Player (AISkirmishPlayer.cpp ↔ skirmish_player.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AISkirmishPlayer.cpp`

### Status: **~75% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| `processBaseBuilding()` | 70-250 | skirmish_player.rs | ⚠️ Partial |
| `buildSpecificAIBuilding()` | 350-450 | skirmish_player.rs | ✅ Implemented |
| `buildAIBaseDefense()` | 500-650 | skirmish_player.rs | ⚠️ Partial |
| Enemy tracking | 700-800 | skirmish_player.rs | ✅ Implemented |
| Skillset selection | 900-1000 | Not found | ❌ **Missing** |

### Critical Issues:

1. **Base Defense Positioning (C++ lines 500-650)**
   - C++ calculates defense angles: `curFrontLeftDefenseAngle`, `curFrontRightDefenseAngle`
   - Uses `SKIRMISH_CENTER`, `SKIRMISH_FLANK`, `SKIRMISH_BACKDOOR` waypoints
   - **Rust has angles but waypoint integration incomplete**

2. **Enemy Player Selection (C++ lines 700-800)**
   ```cpp
   m_currentEnemy = findEnemyPlayer();
   ```
   - C++ tracks single "main enemy" for strategic focus
   - **Rust implementation exists but simplified**

3. **Skillset Selection (C++ lines 900-1000)**
   - Selects from 5 skill sets based on game situation
   - `m_skillsetSelector` determines general abilities
   - **Completely missing in Rust**

---

## 6. AI Group System (AIGroup.cpp ↔ ai_group.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp`

### Status: **~85% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| `add()` | 120-160 | ai_group.rs | ✅ Implemented |
| `remove()` | 165-200 | ai_group.rs | ✅ Implemented |
| `getCenter()` | 220-270 | ai_group.rs | ✅ Implemented |
| `getSpeed()` | 145-170 | ai_group.rs | ✅ Implemented |
| `recompute()` | 350-420 | ai_group.rs | ⚠️ Partial |
| Group commands | 800-1200 | ai_group.rs | ⚠️ Partial |

### Critical Issues:

1. **Formation System (C++ AIGroup.cpp:500-700)**
   - C++ has `FormationID` tracking per unit
   - `getMinMaxAndCenter()` returns formation status
   - **Rust has formation types but not full integration**

2. **Group Pathfinding (C++ AIGroup.cpp:800-1000)**
   - `m_groundPath` shared among all members
   - Path destruction on group changes
   - **Rust implementation exists but simplified**

---

## 7. Squad System (Squad.cpp ↔ squad.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/Squad.cpp`

### Status: **~90% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| `addObject()` | 35-40 | squad.rs | ✅ Implemented |
| `removeObject()` | 45-60 | squad.rs | ✅ Implemented |
| `getAllObjects()` | 65-85 | squad.rs | ✅ Implemented |
| `getLiveObjects()` | 90-110 | squad.rs | ✅ Implemented |
| `squadFromTeam()` | 140-160 | squad.rs | ✅ Implemented |
| `squadFromAIGroup()` | 165-180 | squad.rs | ✅ Implemented |
| `aiGroupFromSquad()` | 185-200 | squad.rs | ✅ Implemented |

### Issues Found:

1. **Xfer/Save-Load (C++ Squad.cpp:200-260)**
   - C++ has complete serialization with version
   - **Rust `Snapshotable` implementation incomplete**

---

## 8. Guard System (AIGuard.cpp ↔ guard.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGuard.cpp`

### Status: **~70% Parity**

### Key Findings:

| State | C++ Lines | Rust Location | Status |
|-------|-----------|---------------|--------|
| `AIGuardMachine` | 150-230 | guard.rs | ✅ Implemented |
| `AIGuardInnerState` | 280-350 | guard.rs | ⚠️ Partial |
| `AIGuardOuterState` | 400-480 | guard.rs | ⚠️ Partial |
| `AIGuardIdleState` | 500-560 | guard.rs | ✅ Implemented |
| `AIGuardReturnState` | 600-680 | guard.rs | ⚠️ Partial |
| `AIGuardAttackAggressorState` | 720-800 | guard.rs | ❌ **Missing** |

### Critical Issues:

1. **Guard Mode Types (C++ AIGuard.cpp:180-220)**
   - `GUARDMODE_NORMAL`
   - `GUARDMODE_GUARD_FLYING_UNITS_ONLY`
   - **Rust has enum but flying-only logic incomplete**

2. **Enter/Hijack Guard (C++ AIGuard.cpp:260-310)**
   - Special guard modes for transport hijacking
   - `isEnterGuard()`, `isHijackGuard()` template checks
   - **Rust has simplified implementation**

3. **Exit Conditions (C++ AIGuard.cpp:80-130)**
   - `ExitConditions::shouldExit()` with bitmask flags
   - `ATTACK_ExitIfExpiredDuration`, `ATTACK_ExitIfOutsideRadius`
   - **Rust implementation exists but incomplete**

---

## 9. Dock System (AIDock.cpp ↔ dock.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIDock.cpp`

### Status: **~85% Parity**

### Key Findings:

| State | C++ Lines | Rust Location | Status |
|-------|-----------|---------------|--------|
| `AIDockMachine` | 50-100 | dock.rs | ✅ Implemented |
| `AIDockApproachState` | 150-220 | dock.rs | ✅ Implemented |
| `AIDockWaitForClearanceState` | 230-300 | dock.rs | ✅ Implemented |
| `AIDockAdvancePositionState` | 310-360 | dock.rs | ✅ Implemented |
| `AIDockMoveToEntryState` | 370-420 | dock.rs | ⚠️ Partial |
| `AIDockProcessDockState` | 500-580 | dock.rs | ⚠️ Partial |

### Issues Found:

1. **Approach Position Indexing (C++ AIDock.cpp:80-100)**
   - `m_approachPosition` tracks queue slot
   - **Rust has Mutex<i32> but race conditions possible**

2. **Timeout Handling (C++ AIDock.cpp:290-310)**
   - `m_enterFrame + 30*LOGICFRAMES_PER_SECOND` timeout
   - **Rust has timeout but frame tracking incomplete**

---

## 10. Turret AI (TurretAI.cpp ↔ turret.rs)

### C++ Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/TurretAI.cpp`

### Status: **~75% Parity**

### Key Findings:

| Feature | C++ Lines | Rust Location | Status |
|---------|-----------|---------------|--------|
| Turret aiming | 100-200 | turret.rs | ✅ Implemented |
| Target acquisition | 250-350 | turret.rs | ⚠️ Partial |
| Turret AI states | 400-600 | turret.rs | ⚠️ Partial |
| `TurretAI` class | 50-100 | turret.rs | ✅ Implemented |

### Critical Issues:

1. **Turret State Machine (C++ TurretAI.cpp:400-600)**
   - Multiple turret states: IDLE, HOLDING, ATTACKING, etc.
   - **Rust has simplified state handling**

---

## Summary of Critical Gaps

### High Priority (Game Breaking):
1. **AIRappelState** - Completely missing, breaks helicopter infantry deployment
2. **RebuildHoleBehavior** - Missing, breaks GLA faction gameplay
3. **Skillset selection** - Missing, affects AI difficulty progression
4. **Garrison priority calculation** - Missing, affects targeting logic

### Medium Priority (Behavioral Differences):
1. **Attack state machine conditionals** - Simplified, may cause incorrect targeting
2. **Path optimization** - Incomplete layer handling, may cause pathfinding issues
3. **Dozer management** - Simplified, may cause base building delays
4. **Defense positioning** - Incomplete waypoint integration

### Low Priority (Polish):
1. **CRC/Xfer serialization** - Incomplete, affects save/load
2. **Debug visualization** - Missing, affects debugging
3. **Container enemy detection** - Missing edge case handling

---

## Recommended Fixes

### Phase 1 - Critical Gameplay
1. Implement `AIRappelState` in ai_states.rs (lines 320-425 of C++)
2. Add `RebuildHoleBehaviorInterface` integration
3. Complete garrison priority calculation in `findClosestEnemy()`
4. Implement skillset selection in `AISkirmishPlayer`

### Phase 2 - Behavioral Parity
1. Complete attack state machine conditionals
2. Add bridge layer support to pathfinding
3. Enhance dozer/worker management
4. Complete defense positioning with waypoint integration

### Phase 3 - Polish
1. Complete CRC/Xfer for all AI classes
2. Add debug visualization support
3. Complete container enemy detection

---

## File References

### C++ Files Analyzed:
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp` (31,963 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIDock.cpp` (25,323 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp` (92,415 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGuard.cpp` (30,634 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGuardRetaliate.cpp` (29,648 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp` (333,599 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPlayer.cpp` (130,963 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AISkirmishPlayer.cpp` (38,525 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp` (256,832 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AITNGuard.cpp` (30,186 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/Squad.cpp` (7,418 bytes)
- `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/TurretAI.cpp` (49,548 bytes)

### Rust Files Analyzed:
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/mod.rs` (79,024 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/ai_core.rs` (7,305 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/ai_player.rs` (130,783 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/ai_states.rs` (127,822 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/ai_group.rs` (42,693 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/skirmish_player.rs` (62,202 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/squad.rs` (14,209 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/guard.rs` (62,586 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/dock.rs` (44,076 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/turret.rs` (72,693 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/pathfind.rs` (49,542 bytes)
- `GeneralsRust/Code/GameEngine/GameLogic/src/ai/pathfind_astar.rs` (26,866 bytes)

---

## Post-Audit Corrections (2026-03-26)

A comprehensive parity audit on 2026-03-26 verified that many items marked as
"missing" or "incomplete" in the original report have since been implemented.

### Items NOW IMPLEMENTED (previously marked missing)

| Section | Item | Original Status | Current Status | Rust Location |
|---------|------|-----------------|----------------|---------------|
| §2 | AIRappelState | ❌ "Completely missing" | ✅ Implemented | `ai/states.rs:3645` |
| §4 | RebuildHoleBehavior integration | ❌ "Missing" | ✅ Implemented | `object/behavior/rebuild_hole_behavior.rs` |
| §5 | Skillset selection | ❌ "Completely missing" | ✅ Implemented | `skirmish_player.rs:1720`, `ai/modules/difficulty_handling.rs` |
| §2 | Container enemy detection | ❌ "Not implemented" | ✅ Implemented | `ai/states.rs:8064` (`find_enemy_in_container`) |
| §8 | AIGuardAttackAggressorState | ❌ "Missing" | ✅ Implemented | `guard.rs:1655` |
| §1 | AiData xfer/save-load | ⚠️ "Missing full field serialization" | ✅ Implemented | `ai/mod.rs:456` (`Snapshot for AiData`) |
| §3 | Bridge pathfinding | ❌ "Not fully implemented" | ✅ Implemented | Path layer + bridge layer transitions present |
| §7 | Squad xfer/save-load | ⚠️ "Incomplete" | ✅ Implemented | Squad snapshotable trait implemented |

### Items that remain partially simplified

| Section | Item | Status | Notes |
|---------|------|--------|-------|
| §1 | findClosestEnemy priority logic | ⚠️ Partial | Container iteration exists but priority weighting may differ |
| §4 | Dozer/worker management | ⚠️ Simplified | Core logic present, advanced queue management simplified |
| §4 | Defense positioning | ⚠️ Partial | Waypoint integration exists but angle calculation may differ |
| §3 | Path optimization | ⚠️ Partial | Layer handling simplified compared to C++ |
| §8 | Guard exit conditions | ⚠️ Partial | Core conditions present, some bitmask flags simplified |
| §10 | Turret state machine | ⚠️ Partial | Core aiming/targeting works, state transitions simplified |
