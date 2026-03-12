# Team Class Parity Analysis

This document compares the C++ Team implementation with the Rust port to identify all missing methods, fields, and functionality.

## Summary

| Component | C++ Lines | Missing in Rust | Priority |
|-----------|-----------|-----------------|----------|
| TeamTemplateInfo | 90-150 | **ALL** - struct missing | CRITICAL |
| PlayerRelationMap | N/A | **ALL** - struct missing | HIGH |
| Team fields | 165-210 | 11 fields missing | CRITICAL |
| Team methods | 220-350 | 25+ methods missing | CRITICAL |
| TeamPrototype fields | 360-400 | 7 fields missing | HIGH |
| TeamPrototype methods | 400-450 | 15+ methods missing | HIGH |
| TeamFactory methods | 460-520 | Mostly implemented | MEDIUM |

---

## 1. TeamTemplateInfo (CRITICAL - ENTIRELY MISSING)

**C++ Location:** Team.h lines 90-150, Team.cpp lines 345-450

The Rust implementation has **NO TeamTemplateInfo struct at all**. This is a critical component that stores all configuration for team creation.

### Missing Struct: TCreateUnitsInfo
```cpp
// C++ Team.h:95-98
typedef struct {
    Int minUnits;
    Int maxUnits;
    AsciiString unitThingName;
} TCreateUnitsInfo;
```
**Priority: CRITICAL** - Required for team unit creation

### Missing Enum: TBehavior
```cpp
// C++ Team.h:103
typedef enum {NORMAL=0, IGNORE_DISTRACTIONS=1, DEAL_AGGRESSIVELY=2} TBehavior;
```
**Priority: MEDIUM** - AI behavior configuration

### Missing Constants
```cpp
// C++ Team.h:102
enum {MAX_UNIT_TYPES = 7};
```
**Priority: CRITICAL** - Used for unit creation arrays

### Missing Fields (TeamTemplateInfo)

| Field | C++ Type | C++ Line | Purpose | Priority |
|-------|----------|----------|---------|----------|
| `m_unitsInfo[MAX_UNIT_TYPES]` | TCreateUnitsInfo[] | 105 | Unit types to create | CRITICAL |
| `m_numUnitsInfo` | Int | 106 | Number of unit entries | CRITICAL |
| `m_homeLocation` | Coord3D | 107 | Spawn location | CRITICAL |
| `m_hasHomeLocation` | Bool | 108 | Location validity flag | HIGH |
| `m_scriptOnCreate` | AsciiString | 109 | Script on creation | CRITICAL |
| `m_scriptOnIdle` | AsciiString | 110 | Script when idle | HIGH |
| `m_initialIdleFrames` | Int | 111 | Frames before idle | MEDIUM |
| `m_scriptOnEnemySighted` | AsciiString | 112 | Script on enemy sight | CRITICAL |
| `m_scriptOnAllClear` | AsciiString | 113 | Script when clear | HIGH |
| `m_scriptOnUnitDestroyed` | AsciiString | 114 | Script on unit death | HIGH |
| `m_scriptOnDestroyed` | AsciiString | 115 | Script on team destroyed | HIGH |
| `m_destroyedThreshold` | Real | 116 | Destruction threshold | HIGH |
| `m_isAIRecruitable` | Bool | 117 | AI recruitment flag | HIGH |
| `m_isBaseDefense` | Bool | 118 | Base defense flag | MEDIUM |
| `m_isPerimeterDefense` | Bool | 119 | Perimeter defense | MEDIUM |
| `m_automaticallyReinforce` | Bool | 120 | Auto-reinforce flag | HIGH |
| `m_transportsReturn` | Bool | 121 | Transport return flag | MEDIUM |
| `m_avoidThreats` | Bool | 122 | Threat avoidance | MEDIUM |
| `m_attackCommonTarget` | Bool | 123 | Common target attack | MEDIUM |
| `m_maxInstances` | Int | 124 | Max team instances | HIGH |
| `m_productionPriority` | mutable Int | 125 | AI priority | CRITICAL |
| `m_productionPrioritySuccessIncrease` | Int | 126 | Priority increase | MEDIUM |
| `m_productionPriorityFailureDecrease` | Int | 127 | Priority decrease | MEDIUM |
| `m_initialTeamAttitude` | AttitudeType | 128 | Initial attitude | HIGH |
| `m_transportUnitType` | AsciiString | 130 | Transport type | HIGH |
| `m_startReinforceWaypoint` | AsciiString | 131 | Reinforce waypoint | HIGH |
| `m_teamStartsFull` | Bool | 132 | Team starts full | MEDIUM |
| `m_transportsExit` | Bool | 133 | Transports exit flag | MEDIUM |
| `m_veterancy` | VeterancyLevel | 134 | Veterancy level | HIGH |
| `m_productionCondition` | AsciiString | 137 | Production condition | CRITICAL |
| `m_executeActions` | Bool | 138 | Execute actions flag | HIGH |
| `m_teamGenericScripts[MAX_GENERIC_SCRIPTS]` | AsciiString[] | 140 | Generic scripts | HIGH |

---

## 2. PlayerRelationMap (HIGH - ENTIRELY MISSING)

**C++ Reference:** Team.cpp uses `PlayerRelationMap` similar to `TeamRelationMap`

```cpp
// C++ Team.cpp:190
PlayerRelationMap *m_playerRelations;
```

### Missing Structure
- Type: `HashMap<i32, Relationship>` (player index -> relationship)
- Purpose: Override player relationships per-team
- **Priority: HIGH** - Required for relationship override system

---

## 3. Team Class Missing Fields

**C++ Location:** Team.h lines 165-210

### Missing Fields

| Field | C++ Type | C++ Line | Purpose | Priority |
|-------|----------|----------|---------|----------|
| `m_proto` | TeamPrototype* | 169 | Prototype reference | CRITICAL (has name field) |
| `m_state` | AsciiString | 179 | AI state name | CRITICAL |
| `m_enteredOrExited` | Bool | 181 | Trigger area flag | HIGH |
| `m_checkEnemySighted` | Bool | 185 | Enemy sighted check | HIGH |
| `m_seeEnemy` | Bool | 186 | Currently seeing enemy | HIGH |
| `m_prevSeeEnemy` | Bool | 187 | Previous frame enemy sight | HIGH |
| `m_wasIdle` | Bool | 190 | Idle last frame | MEDIUM |
| `m_destroyThreshold` | Int | 193 | Destruction threshold | HIGH |
| `m_curUnits` | Int | 194 | Current unit count | HIGH |
| `m_currentWaypoint` | const Waypoint* | 197 | Current waypoint | HIGH |
| `m_shouldAttemptGenericScript[16]` | Bool[] | 200 | Generic script flags | HIGH |
| `m_isRecruitablitySet` | Bool | 203 | Recruitability set flag | MEDIUM |
| `m_isRecruitable` | Bool | 204 | Is recruitable | MEDIUM |
| `m_commonAttackTarget` | ObjectID | 207 | Attack target | HIGH |
| `m_playerRelations` | PlayerRelationMap* | 210 | Player relations | HIGH |
| `m_xferMemberIDList` | std::list<ObjectID> | 212 | Xfer member list | MEDIUM |

### Fields Present in Rust but Incomplete
- `team_relations`: TeamRelationMap - ✅ Present
- `player_relations`: HashMap<i32, Relationship> - ✅ Present but not using PlayerRelationMap type
- `active`: bool - ✅ Present
- `created`: bool - ✅ Present

---

## 4. Team Class Missing Methods

**C++ Location:** Team.h lines 220-350, Team.cpp lines 700-2732

### CRITICAL Priority Methods

| Method | C++ Line | Purpose |
|--------|----------|---------|
| `getPrototype()` | 223 | Get team prototype |
| `getName()` | 298 inline | Get team name (from proto) |
| `getState()` | 280 | Get AI state string |
| `setState()` | 294 | Set AI state string |
| `updateState()` | Team.cpp:1214 | Main update logic (onCreate, enemy sighted, idle, destroyed scripts) |
| `notifyTeamOfObjectDeath()` | Team.cpp:1318 | Notify when member dies |

### HIGH Priority Methods

| Method | C++ Line | Purpose |
|--------|----------|---------|
| `setAttackPriorityName()` | 232 | Set attack priority |
| `getTeamAsAIGroup()` | 286 | Convert to AI group |
| `getTargetableCount()` | 291 | Count targetable members |
| `setRecruitable()` | 300 | Set recruitability |
| `setTeamTargetObject()` | 307 | Set common attack target |
| `getTeamTargetObject()` | 311 | Get common attack target |
| `setEnteredExited()` | 327 | Mark trigger area entry/exit |
| `didEnterOrExit()` | 331 | Check if entered/exited |
| `didAllEnter()` | Team.cpp:1348 | All members entered trigger |
| `didPartialEnter()` | Team.cpp:1392 | Some members entered trigger |
| `didAllExit()` | Team.cpp:2031 | All members exited trigger |
| `didPartialExit()` | Team.cpp:2081 | Some members exited trigger |
| `allInside()` | Team.cpp:2130 | All members inside trigger |
| `someInsideSomeOutside()` | Team.cpp:2246 | Mixed inside/outside |
| `noneInside()` | Team.cpp:2194 | No members inside trigger |
| `tryToRecruit()` | Team.cpp:2348 | Recruit units from other teams |
| `healAllObjects()` | Team.cpp:1068 | Heal all team members |
| `iterateObjects()` | Team.cpp:1077 | Iterate with callback |
| `isIdle()` | Team.cpp:1132 | Check if all idle |
| `unitsEntered()` | 345 | Check trigger area entry |
| `hasAnyBuildFacility()` | Team.cpp:2577 | Check for build facilities |
| `damageTeamMembers()` | Team.cpp:2539 | Damage all members |
| `deleteTeam()` | Team.cpp:2290 | Delete team members |
| `getEstimateTeamPosition()` | Team.cpp:2276 | Get team position estimate |
| `transferUnitsTo()` | Team.cpp:2330 | Transfer units to another team |
| `killTeam()` | Team.cpp:2436 | Kill all team members |
| `evacuateTeam()` | Team.cpp:2407 | Evacuate all containers |
| `getCurrentWaypoint()` | 361 | Get current waypoint |
| `setCurrentWaypoint()` | 362 | Set current waypoint |
| `updateGenericScripts()` | Team.cpp:2595 | Update generic scripts |

### MEDIUM Priority Methods

| Method | C++ Line | Purpose |
|--------|----------|---------|
| `countObjectsByThingTemplate()` | Team.cpp:1020 | Count by template |
| `moveTeamTo()` | Team.cpp:2565 | Move team to destination |

---

## 5. TeamPrototype Missing Fields

**C++ Location:** Team.h lines 360-400

### Missing Fields

| Field | C++ Type | C++ Line | Purpose | Priority |
|-------|----------|----------|---------|----------|
| `m_factory` | TeamFactory* | 382 | Factory reference | MEDIUM |
| `m_owningPlayer` | Player* | 383 | Owning player | CRITICAL |
| `m_productionConditionAlwaysFalse` | Bool | 390 | Production condition flag | HIGH |
| `m_productionConditionScript` | Script* | 391 | Production condition script | HIGH |
| `m_retrievedGenericScripts` | Bool | 393 | Scripts retrieved flag | MEDIUM |
| `m_genericScriptsToRun[16]` | Script*[] | 394 | Generic scripts | HIGH |
| `m_teamTemplate` | TeamTemplateInfo | 396 | Template info | CRITICAL |
| `m_attackPriorityName` | AsciiString | 398 | Attack priority name | MEDIUM |

---

## 6. TeamPrototype Missing Methods

**C++ Location:** Team.h lines 400-450, Team.cpp lines 452-900

### CRITICAL Priority Methods

| Method | C++ Line | Purpose |
|--------|----------|---------|
| `getTemplateInfo()` | 411 | Get TeamTemplateInfo |
| `evaluateProductionCondition()` | 427 | Evaluate production condition |

### HIGH Priority Methods

| Method | C++ Line | Purpose |
|--------|----------|---------|
| `countObjectsByThingTemplate()` | 433 | Count by template |
| `countBuildings()` | 439 | Count buildings |
| `countObjects()` | 444 | Count by KindOfMask |
| `healAllObjects()` | 449 | Heal all objects |
| `iterateObjects()` | 454 | Iterate with callback |
| `countTeamInstances()` | 459 | Count team instances |
| `updateState()` | 464 | Update state, remove empty teams |
| `hasAnyBuildings()` | 469 | Has buildings check (2 overloads) |
| `hasAnyUnits()` | 479 | Has units check |
| `hasAnyObjects()` | 484 | Has objects check |
| `hasAnyBuildFacility()` | 489 | Has build facility |
| `damageTeamMembers()` | 494 | Damage members |
| `moveTeamTo()` | 499 | Move team |
| `teamAboutToBeDeleted()` | 505 | Cleanup before deletion |
| `getGenericScript()` | 507 | Get generic script |
| `increaseAIPriorityForSuccess()` | 510 | Increase AI priority |
| `decreaseAIPriorityForFailure()` | 513 | Decrease AI priority |
| `setAttackPriorityName()` | 516 | Set attack priority |
| `getAttackPriorityName()` | 517 | Get attack priority |
| `friend_setOwningPlayer()` | 502 | Set owning player (friend) |

---

## 7. TeamFactory Missing/Incomplete

**C++ Location:** Team.h lines 460-520

### Mostly Implemented - Minor Issues

| Item | Status | Priority |
|------|--------|----------|
| `initFromSides()` | Needs SidesList* param | MEDIUM |
| Global `TheTeamFactory` | Implemented as lazy_static | LOW |

---

## 8. Xfer (Save/Load) Parity

### Team Xfer Missing Fields

**C++ Team::xfer() lines 2620-2732**

| Field | Xfer'd | Priority |
|-------|--------|----------|
| `m_state` | ❌ Missing | HIGH |
| `m_enteredOrExited` | ❌ Missing | MEDIUM |
| `m_checkEnemySighted` | ❌ Missing | MEDIUM |
| `m_seeEnemy` | ❌ Missing | MEDIUM |
| `m_prevSeeEnemy` | ❌ Missing | MEDIUM |
| `m_wasIdle` | ❌ Missing | MEDIUM |
| `m_destroyThreshold` | ❌ Missing | HIGH |
| `m_curUnits` | ❌ Missing | HIGH |
| `m_currentWaypoint` | ❌ Missing | HIGH |
| `m_shouldAttemptGenericScript[]` | ❌ Missing | HIGH |
| `m_isRecruitablitySet` | ❌ Missing | MEDIUM |
| `m_isRecruitable` | ❌ Missing | MEDIUM |
| `m_commonAttackTarget` | ❌ Missing | HIGH |
| `m_playerRelations` | ❌ Missing | HIGH |

### TeamPrototype Xfer Missing Fields

**C++ TeamPrototype::xfer() lines 820-900**

| Field | Xfer'd | Priority |
|-------|--------|----------|
| Owning player index | ❌ Missing | CRITICAL |
| `m_attackPriorityName` | ❌ Missing (v2+) | MEDIUM |
| `m_productionConditionAlwaysFalse` | ❌ Missing | HIGH |
| `m_teamTemplate` | ❌ Missing | CRITICAL |

---

## 9. Required Types/Dependencies

These types need to be defined or imported for full parity:

| Type | C++ Header | Priority |
|------|------------|----------|
| `AttitudeType` | GameType.h | HIGH |
| `VeterancyLevel` | GameType.h | HIGH |
| `AIGroup` | GameLogic | MEDIUM |
| `PolygonTrigger` | GameLogic | HIGH |
| `Script` | ScriptEngine | HIGH |
| `Waypoint` | TerrainLogic | HIGH |
| `PlayerRelationMap` | (local) | HIGH |
| `Coord3D` | Common | HIGH |
| `ObjectID` | GameLogic | CRITICAL |
| `ThingTemplate` | Common | HIGH |

---

## 10. Implementation Priority Order

### Phase 1 - CRITICAL (Gameplay Blocking)
1. Implement `TeamTemplateInfo` struct with all fields
2. Add `TCreateUnitsInfo` struct
3. Add missing Team fields: `m_proto`, `m_state`, `m_commonAttackTarget`
4. Implement `getPrototype()`, `getName()`, `getState()`, `setState()`
5. Implement `updateState()` - core game loop logic
6. Add proper `PlayerRelationMap` type

### Phase 2 - HIGH (AI/Scripting)
1. Add trigger area methods: `didAllEnter()`, `didPartialEnter()`, etc.
2. Implement `tryToRecruit()`, `setTeamTargetObject()`, `getTeamTargetObject()`
3. Add `notifyTeamOfObjectDeath()`
4. Implement `healAllObjects()`, `killTeam()`, `evacuateTeam()`
5. Add `updateGenericScripts()`, `m_shouldAttemptGenericScript[]`
6. Implement TeamPrototype `evaluateProductionCondition()`

### Phase 3 - MEDIUM (Polish)
1. Implement `getTeamAsAIGroup()`, `getTargetableCount()`
2. Add `moveTeamTo()`, `deleteTeam()`, `transferUnitsTo()`
3. Implement `getEstimateTeamPosition()`, `getCurrentWaypoint()`/`setCurrentWaypoint()`
4. Add priority adjustment methods

### Phase 4 - LOW (Save/Load Completeness)
1. Complete Xfer implementation for all missing fields
2. Add TeamTemplateInfo xfer
3. Complete TeamPrototype xfer with player index

---

## Files Analyzed

| File | Path |
|------|------|
| C++ Header | `GeneralsMD/Code/GameEngine/Include/Common/Team.h` |
| C++ Source | `GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp` |
| Rust Implementation | `GeneralsRust/Code/GameEngine/Common/src/common/rts/team.rs` |

---

*Generated: 2026-03-12*
